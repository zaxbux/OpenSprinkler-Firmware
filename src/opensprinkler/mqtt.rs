use core::fmt;

use serde::{Serialize, Deserialize};

use super::events;

extern crate paho_mqtt as mqtt;

#[derive(Debug, Serialize, Deserialize)]
pub struct MQTTConfig {
    pub enabled: bool,
    pub version: u32,
    /// Broker
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// Use TLS
    pub tls: bool,
}

impl MQTTConfig {
    const PROTOCOL_TCP: &'static str = "tcp";
    const PROTOCOL_SSL: &'static str = "tcp";
    const PROTOCOL_WS: &'static str = "ws";
    const PROTOCOL_WSS: &'static str = "wss";

    pub fn protocol(&self) -> &'static str {
        match self.tls {
            false => Self::PROTOCOL_TCP,
            true => Self::PROTOCOL_SSL,
        }
    }

    pub fn uri(&self) -> String {
        format!("{}://{}:{}", self.protocol(), self.host, self.port)
    }
}

impl Default for MQTTConfig {
    fn default() -> Self {
        MQTTConfig {
            enabled: false,
            version: mqtt::MQTT_VERSION_3_1_1,
            host: String::from(""),
            port: 1883,
            username: String::from(""),
            password: String::from(""),
            tls: false,
        }
    }
}

impl fmt::Display for MQTTConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}:{}", self.protocol(), self.host, self.port)
    }
}

#[derive(Clone)]
pub struct OSMqtt {
    client: Option<mqtt::AsyncClient>,
}

const MQTT_ROOT_TOPIC: &'static str = "opensprinkler";
const MQTT_AVAILABILITY_TOPIC: &'static str = "opensprinkler/availability";
const MQTT_ONLINE_PAYLOAD: &'static str = "online";
const MQTT_OFFLINE_PAYLOAD: &'static str = "offline";

impl OSMqtt {
    pub fn init(&mut self) {
        let id = "OS";
        self.client = Some(mqtt::AsyncClient::new(mqtt::CreateOptionsBuilder::new().client_id(id).finalize()).expect("Failed to init MQTT client"));

        self.client.as_ref().unwrap().set_connected_callback(|client| {
            tracing::trace!("MQTT Connection Callback");
            let tok = client.publish(mqtt::Message::new_retained(MQTT_AVAILABILITY_TOPIC, MQTT_ONLINE_PAYLOAD, 0));
            tok.wait().expect("MQTT Publish: Failed");
        });

        self.client.as_ref().unwrap().set_disconnected_callback(|_, _, reason_code| {
            tracing::trace!("MQTT Disconnnection Callback: {}", reason_code);
        });
    }
    pub fn begin(&self, config: MQTTConfig) {
        tracing::trace!("MQTT Broker: {}", config);

        if self.client.as_ref().unwrap().is_connected() {
            self.client.as_ref().unwrap().disconnect(None);
        }

        let connect_opts = mqtt::ConnectOptionsBuilder::new()
            .mqtt_version(config.version)
            .server_uris(&[config.uri()])
            .user_name(config.username)
            .password(config.password)
            .clean_session(true)
            .will_message(mqtt::Message::new_retained(MQTT_AVAILABILITY_TOPIC, MQTT_OFFLINE_PAYLOAD, 0))
            .finalize();

        //if self._enabled {
        let tok = self.client.as_ref().unwrap().connect(connect_opts);

        tok.wait().expect("MQTT Connect: Connection failed");
        //}
    }
    /*     pub fn enabled(&self) -> bool {
        return self._enabled;
    } */
    pub fn publish<E, S>(&self, event: &E)
    where
        E: events::Event<S>,
        S: serde::Serialize,
    {
        let topic = event.mqtt_topic();
        let payload = event.mqtt_payload_json().expect("Error getting MQTT payload for message");

        tracing::trace!("MQTT Publish: {} {}", topic, payload);

        if !self.client.as_ref().unwrap().is_connected() {
            tracing::trace!("MQTT Publish: Not connected");
            return;
        }

        let tok = self.client.as_ref().unwrap().publish(mqtt::Message::new(topic, payload, 0));
        tok.wait().expect("MQTT Publish: Failed");
    }
    //pub fn r#loop() {}

    pub const fn new() -> OSMqtt {
        return OSMqtt {
            //_enabled: false,
            client: None,
        };
    }
}
