use std::sync::Arc;
use crate::protocol::methods::{channel as protocol_channel};
use crate::protocol::stream::AmqpStream;
use crate::response;
use std::sync::mpsc::{channel, Receiver, Sender};
use log::info;
use crate::protocol::frame::{Method as FrameMethodPayload};
use crate::protocol::frame::MethodFrame;

// todo: to be used
pub struct AmqChannel {
  pub id: i16,
  amqp_stream: Arc<AmqpStream>,
  waiter_channel: (Sender<MethodFrame>, Receiver<MethodFrame>),
  active: bool,
}


impl AmqChannel {
  pub(crate) fn new(id: i16, amqp_stream: Arc<AmqpStream>) -> Self {
    Self {
      id,
      amqp_stream,
      waiter_channel: channel(),
      active: true
    }
  }

  // todo: refactor result to avoid response prefix
  pub fn handle_frame(&self, method: FrameMethodPayload) -> response::Result<()> {
    match method {
      FrameMethodPayload::ChanOpenOk(payload) => {
        info!("Received open ok method {:?}", payload)
      },
      _ => {
        panic!("Received unknown method");
      }
    }
    Ok(())
  }

  pub fn open(&self) -> response::Result<()> {
    info!("Invoking Open");
    let mut stream_writer = self.amqp_stream.writer.lock().unwrap();
    stream_writer.invoke(self.id, protocol_channel::Open::default())?;
    self.wait_for_response()?;

    Ok(())
  }

  pub fn flow(&mut self, active: bool) -> response::Result<()> {
    info!("Invoking Flow");
    if self.active == active {
      return Ok(())
    }

    let mut stream_writer = self.amqp_stream.writer.lock().unwrap();
    stream_writer.invoke(self.id, protocol_channel::Flow {
      active: active as u8
    })?;
    self.wait_for_response()?;
    self.active = active;

    Ok(())
  }

  fn wait_for_response(&self) -> response::Result<MethodFrame> {
    Ok(self.waiter_channel.1.recv()?)
  }
}