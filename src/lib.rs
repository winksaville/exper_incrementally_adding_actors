use crossbeam_channel::{Sender, Receiver, unbounded};

pub type BoxMsgAny = Box<dyn std::any::Any + Send>;

#[derive(Debug, Clone)]
pub struct ThingChannel {
    pub tx: Sender<BoxMsgAny>,
    pub rx: Receiver<BoxMsgAny>,
}

impl ThingChannel {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self {
            tx,
            rx,
        }
    }
}

struct MsgInc;

#[derive(Debug)]
pub struct Thing {
    pub name: String,
    pub channel: ThingChannel,
    pub counter: i32,
}

impl Thing {
    pub fn new(name: &str, channel: ThingChannel) -> Self {
        Self {
            name: name.to_string(),
            channel,
            counter: 0,
        }
    }

    pub fn increment(&mut self) {
        self.counter += 1;
        println!("Thing::increment: counter={}", self.counter);
    }

    pub fn process_msg_any(&mut self, msg: BoxMsgAny) {
        if msg.downcast_ref::<MsgInc>().is_some() {
           self.increment()
        } else {
            println!("Thing.prossess_msg_any: Uknown msg");
        }
    }
}

#[allow(unused)]
#[derive(Debug)]
pub enum ManagedThing {
    TheThing(Thing),
    ItsChannel(ThingChannel),
}

#[derive(Debug)]
pub struct Manager {
    things: Vec<ManagedThing>,
}

impl Manager {
    pub fn new() -> Manager {
        Self {
            things: vec![],
        }
    }

    pub fn add_thing(&mut self, thing: Thing) -> usize {
        let idx = self.things.len();
        self.things.push(ManagedThing::TheThing(thing));
        idx
    }

    pub fn get_tx_for_thing(&self, handle: usize) -> Option<Sender<BoxMsgAny>> {
        if let Some(mt) = self.things.get(handle) {
            match mt {
                ManagedThing::TheThing(_) => None,
                ManagedThing::ItsChannel(t) => {
                    Some(t.tx.clone())
                }
            }
        } else {
            None
        }
    }

    // Need to make thread safe
    pub fn own_thing(&mut self, handle: usize) -> Option<Thing> {
        if let Some(mt) = self.things.get(handle) {
            match mt {
                ManagedThing::TheThing(thing) => {
                    let tc = thing.channel.clone();
                    drop(thing);
                    drop(mt);

                    // Replace the thing with it's channel
                    let mt = std::mem::replace(&mut self.things[handle], ManagedThing::ItsChannel(tc));
                    match mt {
                        ManagedThing::TheThing(thing) => {
                            Some(thing)
                        }
                        // This should/cannot ever happen!!
                        _ => {
                            // Panic on debug builds, None on release builds
                            debug_assert!(true, "Manager::own_thing: swap returned TheThing but this should never happen");
                            None
                        }
                    }
                }
                ManagedThing::ItsChannel(_) => {
                    // Already owned
                    None
                }
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let thing = Thing::new("t1", ThingChannel::new());
        println!("main: thing1={thing:?}");
        let mut manager = Manager::new();
        println!("main: new manager={manager:?}");
        let t1_handle = manager.add_thing(thing);
        println!("main: t1_handle={t1_handle} manager={manager:?}");

        let mut t1 = manager.own_thing(t1_handle).unwrap();
        let t1_tx = manager.get_tx_for_thing(t1_handle).unwrap();

        t1_tx.send(Box::new(MsgInc{})).unwrap();
        let msg = t1.channel.rx.recv().unwrap();
        t1.process_msg_any(msg);
        println!("main: t1={t1:?}");
    }
}
