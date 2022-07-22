use libc::{c_char, c_int};
extern crate paho_mqtt as mqtt;
use tracing::debug;

use crate::utils::get_str_from_cstr;

#[derive(Clone)]
pub struct OSMqtt {
    _enabled: bool,
    _client: Option<mqtt::Client>,
}

const MQTT_ROOT_TOPIC: &'static str = "opensprinkler";
const MQTT_AVAILABILITY_TOPIC: &'static str = "opensprinkler/availability";
const MQTT_ONLINE_PAYLOAD: &'static str = "online";
const MQTT_OFFLINE_PAYLOAD: &'static str = "offline";

impl OSMqtt {
    pub fn init(&mut self) {
        let id = "OS";
        self._client = Some(
            mqtt::Client::new(mqtt::CreateOptionsBuilder::new().client_id(id).finalize())
                .expect("Failed to init MQTT client"),
        );

        self._client
            .as_ref()
            .unwrap()
            .set_connected_callback(|client| {
                debug!("MQTT Connection Callback");
                let tok = client.publish(mqtt::Message::new_retained(
                    MQTT_AVAILABILITY_TOPIC,
                    MQTT_ONLINE_PAYLOAD,
                    0,
                ));
                tok.wait().expect("MQTT Publish: Failed");
            });

        self._client
            .as_ref()
            .unwrap()
            .set_disconnected_callback(|_, _, reason_code| {
                debug!("MQTT Disconnnection Callback: {}", reason_code);
            });
    }
    pub fn begin(
        &self,
        host: *const c_char,
        port: c_int,
        username: *const c_char,
        password: *const c_char,
        enable: bool,
    ) {
        let host_str = get_str_from_cstr(host);
        let username_str = get_str_from_cstr(username);
        let password_str = get_str_from_cstr(password);
        let enable_str = if enable { "Enabled" } else { "Disabled" };
        debug!(
            "MQTT Begin: Config ({}:{} {}) {}",
            host_str, port, username_str, enable_str
        );

        if self._client.as_ref().unwrap().is_connected() {
            self._client.as_ref().unwrap().disconnect(None);
        }

        let connect_opts = mqtt::ConnectOptionsBuilder::new()
            .mqtt_version(mqtt::MQTT_VERSION_3_1_1)
            .server_uris(&[format!("tcp://{}:{}", host_str, port).to_string()])
            .user_name(username_str)
            .password(password_str)
            .clean_session(true)
            .will_message(mqtt::Message::new_retained(
                MQTT_AVAILABILITY_TOPIC,
                MQTT_OFFLINE_PAYLOAD,
                0,
            ))
            .finalize();

        if self._enabled {
            let tok = self._client.as_ref().unwrap().connect(connect_opts);

            tok.wait().expect("MQTT Connect: Connection failed");
        }
    }
    pub fn enabled(&self) -> bool {
        return self._enabled;
    }
    pub fn publish(&self, topic: *const c_char, payload: *const c_char) {
        let topic_str = get_str_from_cstr(topic);
        let payload_str = get_str_from_cstr(payload);
        debug!("MQTT Publish: {} {}", topic_str, payload_str);

        if !self._client.as_ref().unwrap().is_connected() {
            debug!("MQTT Publish: Not connected");
            return;
        }

        let tok =
            self._client
                .as_ref()
                .unwrap()
                .publish(mqtt::Message::new(topic_str, payload_str, 0));
        tok.wait().expect("MQTT Publish: Failed");
    }
    pub fn r#loop() {}

    pub const fn new() -> OSMqtt {
        return OSMqtt {
            _enabled: false,
            _client: None,
        };
    }
}
