use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use log::{warn};
use bevy::prelude::*;
use crate::server::{Server, GenericParser, Enveloppe};

#[derive(Default, Debug)]
pub struct WebSocketServer {}

impl Plugin for WebSocketServer {
    fn build(&self, app: &mut AppBuilder) {
        let server = Server::new();
        let router = Arc::new(Mutex::new(GenericParser::new()));
        let map = HashMap::<String, Vec<Enveloppe>>::new();
        app
            .insert_resource(server)
            .insert_resource(router)
            .insert_resource(map)
            .add_system(consume_messages.system())
        ;
    }
}

fn consume_messages(
    mut server: ResMut<Server>,
    mut hmap: ResMut<HashMap::<String, Vec<Enveloppe>>>
) {
    if !server.is_running() {
        return;
    }

    while let Some((handle, raw_ev)) = server.recv() {
        println!("consuming message from {:?}", handle);
        if let Ok(enveloppe) = serde_json::from_reader::<std::io::Cursor<Vec<u8>>, Enveloppe>(std::io::Cursor::new(raw_ev)) {
            let tp = enveloppe.message_type.to_string();
            let mut v = if let Some(x) = hmap.remove(&tp) {
                x
            } else { Vec::new() };
            v.push(enveloppe.clone());
            hmap.insert(tp, v);
        } else {
            warn!("failed to deserialize message from {:?}", handle);
            continue
        }
    }
}

fn add_message_consumer<T>(
    key: Local<String>,
    mut hmap: ResMut<HashMap::<String, Vec<Enveloppe>>>,
    router: Res<Arc<Mutex<GenericParser>>>,
    mut queue: EventWriter<T>
)
where T: Send + Sync + 'static {
    if let Some(values) = hmap.remove(&*key) {
        for v in values {
            let enveloppe = router.lock().unwrap().parse_enveloppe(&v);
            match enveloppe {
                Ok(dat) => {
                    match GenericParser::to_concrete_type::<T>(dat) {
                        Ok(msg) => {
                            queue.send(msg);
                        },
                        Err(e) => {
                            warn!("failed to downcast : {}", e);
                        }
                    }

                },
                Err(e) => {
                    warn!("failed to parse type enveloppe : {}", e);
                    continue
                }
            };

        }
    }
}

pub trait WsMessageInserter {
    fn register_message_type<T>(
        &mut self,
        tag: &str
    ) -> &mut Self
    where T: Send + Sync + serde::de::DeserializeOwned + 'static;
}


impl WsMessageInserter for AppBuilder {
    fn register_message_type<T>(
        &mut self,
        tag: &str
    ) -> &mut Self
    where T: Send + Sync + serde::de::DeserializeOwned + 'static {
        self.add_event::<T>();
        let router = self
            .app
            .world
            .get_resource::<Arc<Mutex<GenericParser>>>()
            .expect("cannot register message before WebSocketServer initialization");
        router
            .lock()
            .unwrap()
            .insert_type::<T>(tag);

        self.add_system(add_message_consumer::<T>.system().config(|params| {
            params.0 = Some(tag.to_string());
        }));
        self
    }
}
