use super::events::{self, mqtt::MQTTConfig};

extern crate paho_mqtt as mqtt;

#[derive(Clone)]
pub struct Mqtt {
    client: Option<mqtt::AsyncClient>,
    config: MQTTConfig,
}

impl Mqtt {
    pub fn new() -> Self {
        Self { client: None, config: MQTTConfig::default() }
    }

    pub fn setup(&mut self, config: &MQTTConfig) -> mqtt::errors::Result<()> {
        self.config = config.clone();

        let client = mqtt::AsyncClient::new(Self::get_create_options().finalize())?;

        client.set_connected_callback(self.cb_connected());
        client.set_disconnected_callback(self.cb_disconnected());

        self.client = Some(client);

        Ok(())
    }

    fn get_create_options() -> mqtt::CreateOptionsBuilder {
        mqtt::CreateOptionsBuilder::new().client_id("OS")
    }

    fn get_connect_options(&self) -> Option<mqtt::ConnectOptionsBuilder> {
        tracing::trace!("MQTT Broker: {}", self.config);

        let offline_message = mqtt::Message::new_retained(self.resolve_topic(&self.config.availability_topic), self.config.offline_payload.as_bytes(), 0);

        let mut builder = mqtt::ConnectOptionsBuilder::new();
        builder.mqtt_version(self.config.version).clean_session(true).will_message(offline_message);

        if let Some(uri) = self.config.uri() {
            builder.server_uris(&[uri]);
        }

        if let Some(ref username) = self.config.username {
            builder.user_name(username);
        }

        if let Some(ref password) = self.config.password {
            builder.password(password);
        }

        Some(builder)
    }

    fn cb_connected(&self) -> Box<mqtt::ConnectedCallback> {
        let online_message = mqtt::Message::new_retained(self.resolve_topic(&self.config.availability_topic), self.config.online_payload.as_bytes(), 0);
        Box::new(move |client: &mqtt::AsyncClient| -> () {
            tracing::trace!("Connection Callback");
            let _ = client.publish(online_message.clone());
        })
    }

    fn cb_disconnected(&self) -> Box<mqtt::DisconnectedCallback> {
        Box::new(|_: &mqtt::AsyncClient, _: mqtt::Properties, reason_code: mqtt::ReasonCode| {
            tracing::trace!("MQTT Disconnection Callback: {}", reason_code);
        })
    }

    fn resolve_topic(&self, topic: &str) -> String {
        let mut full_topic = self.config.root_topic.clone();
        full_topic.push('/');
        full_topic.push_str(&topic);

        full_topic
    }

    pub fn is_connected(&self) -> bool {
        if let Some(ref client) = self.client {
            return client.is_connected();
        }

        false
    }

    pub fn connect(&self) -> Option<mqtt::Token> {
        if let Some(ref client) = self.client {
            if let Some(options) = self.get_connect_options() {
                return Some(client.connect(options.finalize()));
            }
        }

        None
    }

    pub fn disconnect(&self) -> Result<Option<mqtt::Token>, mqtt::Error> {
        if let Some(ref client) = self.client {
            return Ok(Some(client.disconnect(None)));
        }

        Ok(None)
    }

    pub fn publish<E, S>(&self, event: &E) -> Result<Option<mqtt::DeliveryToken>, mqtt::Error>
    where
        E: events::Event<S>,
        S: serde::Serialize,
    {
        if let Some(ref client) = self.client {
            if let Ok(payload) = event.mqtt_payload_json() {
                let tok = client.publish(mqtt::Message::new(self.resolve_topic(&event.mqtt_topic()), payload, 0));
                return Ok(Some(tok));
            }
        }

        Ok(None)
    }
}
