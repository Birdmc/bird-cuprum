use std::borrow::Borrow;
use tokio::*;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use cubic_protocol::packet::{InputPacketBytes, PacketReadable, PacketWritable};
use cubic_protocol::packet_bytes::{InputPacketBytesPrepared, OutputPacketBytesVec};
use cubic_protocol::packet_default::Handshaking;
use cubic_protocol::types::VarInt;
use cubic_protocol_server::read::ReadStreamQueue;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use crate::config::{CuprumConfig, CuprumServerConfig};
use crate::route::IpRoute;

const READ_STREAM_QUEUE_SIZE: usize = 128;
const BUFFER_SIZE: usize = 128;

pub fn run_server(config: CuprumConfig) -> Vec<task::JoinHandle<anyhow::Result<()>>> {
    IpRoute::new_routes(config)
        .into_iter()
        .map(|(port, route)| {
            tokio::spawn(async move { run_listener(port, route).await })
        })
        .collect()
}

async fn run_listener(port: u16, route: IpRoute) -> anyhow::Result<()> {
    let listener = net::TcpListener::bind(
        format!("0.0.0.0:{}", port)
    ).await?;
    loop {
        let (stream, addr) = listener.accept().await?;
        let route = route.clone();
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
            match chosen_server {
                Some(chosen_server) => {
                    log::debug!(
                        "Connecting to the main server: {}",
                        chosen_server.server
                    );
                    match net::TcpStream::connect(chosen_server.server.as_str()).await {
                        Ok(connection) => run_as_proxy(
                            chosen_server,
                            connection,
                            read_queue,
                            write,
                            handshake_packet
                        ).await,
                        Err(_) => failed_as_proxy(
                            chosen_server,
                            read_queue,
                            write,
                            handshake_packet
                        ).await,
                    }
                }
                None => {
                    log::debug!("Can not find server: {}", handshake_packet.server_address);
                    write.shutdown().await.map_err(|e| e.into())
                },
            }
        });
    }
}

async fn proxying<const BUFFER_SIZE: usize>(
    mut read: OwnedReadHalf,
    mut write: OwnedWriteHalf,
) {
    let mut buffer = [0_u8; BUFFER_SIZE];
    loop {
        match read.read(&mut buffer).await {
            Ok(0) | Err(_) => {
                write.shutdown().await;
                break;
            }
            Ok(size) => {
                write.write_all(&buffer[0..size]).await
            }
        };
    }
}

async fn run_as_proxy<const BUFFER_SIZE: usize>(
    server_config: Arc<CuprumServerConfig>,
    server_connection: TcpStream,
    read: ReadStreamQueue<BUFFER_SIZE>,
    write: OwnedWriteHalf,
    handshake: Handshaking,
) -> anyhow::Result<()> {
    log::debug!("Connecting to the main server - Success");
    let (read, additional) = read.close();
    let (server_read, mut server_write) =
        server_connection.into_split();
    tokio::spawn(async move {
        proxying::<BUFFER_SIZE>(server_read, write).await
    });
    let mut handshake_bytes = OutputPacketBytesVec::new();
    <Handshaking as PacketWritable>::write(handshake, &mut handshake_bytes).await?;
    let mut end_bytes = OutputPacketBytesVec::new();
    <VarInt as PacketWritable>::write(VarInt::from(handshake_bytes.data.len() as i32), &mut end_bytes).await?;
    server_write.write_all(end_bytes.data.as_slice()).await;
    server_write.write_all(handshake_bytes.data.as_slice()).await;
    drop(handshake_bytes);
    drop(end_bytes);
    server_write.write_all(additional.borrow()).await;
    proxying::<BUFFER_SIZE>(read, server_write).await;
    Ok(())
}

async fn failed_as_proxy<const BUFFER_SIZE: usize>(
    server_config: Arc<CuprumServerConfig>,
    read: ReadStreamQueue<BUFFER_SIZE>,
    write: OwnedWriteHalf,
    handshake: Handshaking,
) -> anyhow::Result<()> {
    log::debug!("Connecting to the main server - Failed");
    todo!("Implement")
}