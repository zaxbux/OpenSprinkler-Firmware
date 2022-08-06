use super::events;

extern crate paho_mqtt as mqtt;

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
    pub fn begin(&self, config: events::mqtt::MQTTConfig) {
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
