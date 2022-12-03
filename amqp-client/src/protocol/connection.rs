use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use log::info;
use amqp_protocol::types::Property;
use crate::protocol::channel::AmqChannel;
use crate::protocol::frame::{AmqFrame};
use crate::protocol::stream::{AmqpStream};
use crate::response;
use crate::utils::IdAllocator;

pub mod constants;
pub mod methods;

pub struct AmqConnection {
  pub options: ConnectionOpts,
  amqp_stream: Arc<AmqpStream>,
  channels: Arc<Mutex<HashMap<i16,Arc<AmqChannel>>>>,
  id_allocator: IdAllocator,
}

#[derive(Clone)]
pub struct ConnectionOpts {
  pub host: String,
  pub port: u16,
  pub login: String,
  pub password: String,
  pub vhost: String,
}

impl AmqConnection {
  pub fn new(host: String, port: u16, login: String, password: String, vhost: String) -> Self {
    let connection_url = format!("{}:{}", host.clone(), port);
    let options = ConnectionOpts { host, port, login, password, vhost };

    AmqConnection {
      amqp_stream: Arc::new(AmqpStream::new(connection_url)),
      channels: Arc::new(Mutex::new(HashMap::new())),
      id_allocator: IdAllocator::new(),
      options
    }
  }

  pub fn connect(&mut self) -> response::Result<()> {
    use crate::protocol::connection::constants::PROTOCOL_HEADER;
    use crate::protocol::connection::methods as conn_methods;

    let mut reader = self.amqp_stream.reader.lock().unwrap();
    let mut writer = self.amqp_stream.writer.lock().unwrap();
    writer.send_raw(&PROTOCOL_HEADER)?;

    let frame = reader.next_method_frame()?;
    // todo: write some helper for a such common case
    let start_method: conn_methods::Start = frame.body.try_into()?;

    // todo: provide some meaningful values
    let mut client_properties = HashMap::new();
    client_properties.insert("product".to_string(), Property::LongStr("simpleapp123123asd ".to_string()));
    client_properties.insert("platform".to_string(), Property::LongStr("Erlang/OTP 24.3.4".to_string()));
    client_properties.insert("copyright".to_string(), Property::LongStr("param-pam-pamqweqwe".to_string()));
    client_properties.insert("information".to_string(), Property::LongStr("Licensed under the MPL 2.0".to_string()));

    // todo: use values from conn options
    let start_ok_method = conn_methods::StartOk {
      properties: client_properties,
      mechanism: "PLAIN".to_string(),
      response: format!("\x00{}\x00{}", self.options.login.as_str(), self.options.password),
      locale: "en_US".to_string()
    };
    writer.invoke(0, start_ok_method)?;

    let frame = reader.next_method_frame()?;
    let tune_method: conn_methods::Tune = frame.body.try_into()?;

    // todo: use values from conn options
    let tune_ok_method = conn_methods::TuneOk {
      chan_max: tune_method.chan_max,
      frame_max: tune_method.frame_max,
      heartbeat: tune_method.heartbeat
    };
    writer.invoke(0, tune_ok_method)?;

    let open_method = conn_methods::Open {
      vhost: self.options.vhost.clone(),
      ..conn_methods::Open::default()
    };
    writer.invoke(0, open_method)?;
    let frame = reader.next_method_frame()?;
    let open_ok_method: conn_methods::OpenOk = frame.body.try_into()?;

    info!("Received OpenOk method {:?}", open_ok_method);
    // todo: implement
    // self.start_listener()?;
    drop(reader);
    drop(writer);

    self.start_listener();
    Ok(())
  }

  pub fn create_channel(&mut self) -> response::Result<Arc<AmqChannel>> {
    let id = self.id_allocator.allocate();
    let chan = AmqChannel::new(id, self.amqp_stream.clone());
    let chan = Arc::new(chan);
    self.channels.lock().unwrap().insert(chan.id, chan.clone());
    chan.open()?;

    Ok(chan)
  }

  pub fn start_listener(&self) {
    let reader = self.amqp_stream.reader.clone();
    let channels = self.channels.clone();

    std::thread::spawn(move || {
      let mut reader = reader.lock().unwrap();

      while let Ok(frame) = reader.next_frame() {
        match frame {
          AmqFrame::Method(method_frame) => {
            println!("Received method frame {:?} {:?}", method_frame.class_id, method_frame.method_id);
            channels.lock().unwrap()[&method_frame.chan].handle_frame(method_frame).unwrap();
          },
          _ => {
            panic!("Unsupported frame type")
          }
        }
      }
    });
  }
}