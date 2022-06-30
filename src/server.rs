use std::borrow::Borrow;
use std::net::SocketAddr;
use tokio::*;
use std::sync::Arc;
use bird_chat::component::TextComponent;
use bird_protocol::packet::{InputPacketBytes, PacketReadable, PacketWritable};
use bird_protocol::packet_bytes::OutputPacketBytesVec;
use bird_protocol::packet_default::{ClientStatusPacket, HandshakeNextState, Handshaking, LoginDisconnect, StatusPong, StatusResponse};
use bird_protocol::types::VarInt;
use bird_protocol_server::read::ReadStreamQueue;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use crate::config::{CuprumConfig, CuprumConfigEx, CuprumServerConfig};
use crate::route::IpRoute;

const READ_STREAM_QUEUE_SIZE: usize = 128;

pub fn run_server(config: CuprumConfig) -> Vec<task::JoinHandle<anyhow::Result<()>>> {
    let CuprumConfig { servers, ex } = config;
    let external_config = Arc::new(ex);
    IpRoute::new_routes(servers)
        .into_iter()
        .map(|(port, route)| {
            let external_config = external_config.clone();
            tokio::spawn(async move { run_listener(port, route, external_config).await })
        })
        .collect()
}

async fn run_listener(port: u16, route: IpRoute, external_config: Arc<CuprumConfigEx>) -> anyhow::Result<()> {
    let listener = net::TcpListener::bind(
        format!("0.0.0.0:{}", port)
    ).await?;
    loop {
        let (stream, addr) = listener.accept().await?;
        let route = route.clone();
        let external_config = external_config.clone();
        tokio::spawn(async move {
            let (read, mut write) = stream.into_split();
            let mut read_queue = ReadStreamQueue::<READ_STREAM_QUEUE_SIZE>::new(read);
            read_queue.next_packet().await?;
            let _ = read_queue.take_byte().await?; // Packet ID byte
            let handshake_packet =
                <Handshaking as PacketReadable>::read(&mut read_queue).await?;
            let chosen_server = route.choose_server_config(
                &handshake_packet.server_address
            );
            let res = match chosen_server {
                Some(chosen_server) => {
                    log::debug!(
                        "Connecting to the main server: {}",
                        chosen_server.server
                    );
                    match net::TcpStream::connect(chosen_server.server.as_str()).await {
                        Ok(connection) => run_as_proxy(
                            chosen_server,
                            external_config,
                            connection,
                            read_queue,
                            write,
                            handshake_packet,
                            addr,
                        ).await,
                        Err(_) => failed_as_proxy(
                            chosen_server,
                            read_queue,
                            write,
                            handshake_packet,
                        ).await,
                    }
                }
                None => {
                    log::debug!("Can not find server: {}", handshake_packet.server_address);
                    write.shutdown().await.map_err(|e| e.into())
                }
            };
            match res {
                Err(e) => {
                    log::debug!("Error occured: {}", e);
                    Err(e)
                }
                Ok(val) => Ok(val)
            }
        });
    }
}

async fn proxying(
    mut read: OwnedReadHalf,
    mut write: OwnedWriteHalf,
    buffer_size: usize,
) {
    let mut buffer = Vec::with_capacity(buffer_size);
    unsafe { buffer.set_len(buffer_size); }
    loop {
        match read.read(buffer.as_mut_slice()).await {
            Ok(0) | Err(_) => {
                let _ = write.shutdown().await;
                break;
            }
            Ok(size) => {
                let _ = write.write_all(&buffer[0..size]).await;
            }
        };
    }
}

async fn send_packet(write: &mut OwnedWriteHalf, packet: impl PacketWritable) -> anyhow::Result<()> {
    let mut packet_bytes = OutputPacketBytesVec::new();
    packet.write(&mut packet_bytes).await?;
    let mut length_bytes = OutputPacketBytesVec::new();
    <VarInt as PacketWritable>::write(VarInt::from(packet_bytes.data.len() as i32), &mut length_bytes).await?;
    log::debug!("Sending bytes: {:?} {:?}", &length_bytes.data, &packet_bytes.data);
    write.write_all(length_bytes.data.as_slice()).await?;
    write.write_all(packet_bytes.data.as_slice()).await?;
    Ok(())
}

async fn run_as_proxy<const RS_QUEUE_BUFFER_SIZE: usize>(
    server_config: Arc<CuprumServerConfig>,
    config: Arc<CuprumConfigEx>,
    server_connection: TcpStream,
    read: ReadStreamQueue<RS_QUEUE_BUFFER_SIZE>,
    write: OwnedWriteHalf,
    mut handshake: Handshaking,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    log::debug!("Connecting to the main server - Success");
    let (read, additional) = read.close();
    let (server_read, mut server_write) =
        server_connection.into_split();
    let buffer_size = config.buffer_size;
    tokio::spawn(async move {
        proxying(server_read, write, buffer_size).await
    });
    handshake.server_address = server_config
        .ip_forwarding
        .handshake_string(
            handshake.server_address,
            server_config.clone(),
            addr,
        );
    send_packet(&mut server_write, handshake).await?;
    let _ = server_write.write_all(additional.borrow()).await?;
    proxying(read, server_write, config.buffer_size).await;
    Ok(())
}

async fn failed_as_proxy<const BUFFER_SIZE: usize>(
    server_config: Arc<CuprumServerConfig>,
    mut read: ReadStreamQueue<BUFFER_SIZE>,
    mut write: OwnedWriteHalf,
    handshake: Handshaking,
) -> anyhow::Result<()> {
    log::debug!("Connecting to the main server - Failed");
    match handshake.next_state {
        HandshakeNextState::Login => send_packet(
            &mut write,
            LoginDisconnect {
                reason: TextComponent::new(server_config.offline_kick.clone()).into()
            },
        ).await,
        HandshakeNextState::Status => {
            loop {
                read.next_packet().await?;
                let packet = ClientStatusPacket::read(&mut read).await?;
                match packet {
                    ClientStatusPacket::StatusPing(ping) => send_packet(
                        &mut write,
                        StatusPong {
                            payload: ping.payload
                        },
                    ).await,
                    ClientStatusPacket::StatusRequest(_) => send_packet(
                        &mut write,
                        StatusResponse {
                            response: server_config.offline_status.clone()
                        },
                    ).await
                }?;
            }
        }
    }
}