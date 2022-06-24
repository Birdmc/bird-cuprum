use std::collections::HashMap;
use std::sync::Arc;

use crate::config::CuprumServerConfig;

#[derive(Clone)]
pub struct IpRoute {
    pub default: Option<Arc<CuprumServerConfig>>,
    pub routes: Arc<HashMap<String, Arc<CuprumServerConfig>>>,
}

impl IpRoute {
    pub fn new_routes(routes: Vec<CuprumServerConfig>) -> HashMap<u16, IpRoute> {
        let mut ip_routes = HashMap::new();
        routes
            .into_iter()
            .for_each(|config| {
                let ip_route = match ip_routes.get_mut(&config.listen) {
                    Some(ip_route) => ip_route,
                    None => {
                        ip_routes.insert(
                            config.listen,
                            IpRoute {
                                default: None,
                                routes: Arc::new(HashMap::new()),
                            },
                        );
                        ip_routes.get_mut(&config.listen).unwrap()
                    }
                };
                let config = Arc::new(config);
                match config.host.is_empty() {
                    true => ip_route.default = Some(config),
                    false => {
                        Arc::get_mut(&mut ip_route.routes)
                            .unwrap()
                            .insert(config.host.clone(), config);
                    }
                }
            });
        ip_routes
    }

    pub fn choose_server_config(&self, ip: &String) -> Option<Arc<CuprumServerConfig>> {
        match self.routes.get(ip) {
            Some(config) => Some(config.clone()),
            None => self.default.clone()
        }
    }
}