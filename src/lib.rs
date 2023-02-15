use std::{
    fmt::Debug,
    thread::{self, JoinHandle},
};

use crossbeam_channel::{unbounded, Receiver, Select, Sender};

pub type BoxMsgAny = Box<dyn std::any::Any + Send>;

trait Actor: Send + Debug {
    fn process_msg_any(&mut self, reply_tx: Option<&Sender<BoxMsgAny>>, msg: BoxMsgAny);
    fn name(&self) -> &str;
    fn done(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct ThingChannel {
    pub tx: Sender<BoxMsgAny>,
    pub rx: Receiver<BoxMsgAny>,
}

impl Default for ThingChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl ThingChannel {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self { tx, rx }
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
}

impl Actor for Thing {
    fn process_msg_any(&mut self, _reply_tx: Option<&Sender<BoxMsgAny>>, msg: BoxMsgAny) {
        if msg.downcast_ref::<MsgInc>().is_some() {
            self.increment()
        } else {
            println!("Thing.prossess_msg_any: Uknown msg");
        }
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn done(&self) -> bool {
        false
    }
}

#[allow(unused)]
#[derive(Debug)]
struct ActorsExecutor {
    pub name: String,
    pub actors: Vec<Box<dyn Actor>>,
    pub receivers: Vec<Receiver<BoxMsgAny>>,
}

#[allow(unused)]
impl ActorsExecutor {
    pub fn start(name: &str) -> (JoinHandle<()>, Sender<BoxMsgAny>) {
        let (ctrl_tx, ctrl_rx) = unbounded::<BoxMsgAny>();

        let mut ae = Box::new(Self {
            name: name.to_string(),
            actors: Vec::new(),
            receivers: Vec::new(),
        });

        let join_handle = thread::spawn(move || {
            println!("AE:{}:+", ae.name);

            // The 0th selector will always be the AeActor
            let ae_actor = Box::new(AeActor::new());
            ae.actors.push(ae_actor);
            ae.receivers.push(ctrl_rx);
            let mut selector = Select::new();
            let oper_idx = selector.recv(&ae.receivers[0]);
            assert_eq!(oper_idx, ae.actors.len() - 1);

            let mut done = false;
            while !done {
                println!("AE:{}: TOL", ae.name);
                let oper = selector.select();
                let oper_idx = oper.index();
                let actor = &mut ae.actors[oper_idx];
                let rx = &ae.receivers[oper_idx];
                if let Ok(msg) = oper.recv(rx).map_err(|why| {
                    // TODO: What to do on errors in general
                    if oper_idx == 0 {
                        // Error on AeActor, we'll be done
                        println!(
                            "AE:{}: {} error on recv: {why} `done = true`",
                            ae.name,
                            actor.name()
                        );
                        done = true;
                    } else {
                        // panic
                        todo!("AE:{}: {} error on recv: {why}", ae.name, actor.name())
                    }
                }) {
                    actor.process_msg_any(None, msg);
                    if actor.done() {
                        if oper_idx == 0 {
                            println!(
                                "AE:{}: {} reports done, stopping the AE",
                                ae.name,
                                actor.name()
                            );
                            done = actor.done();
                        } else {
                            // panic
                            todo!(
                                "AE:{}: {} reported done, what to do?",
                                ae.name,
                                actor.name()
                            )
                        }
                    }
                };
            }

            // TODO: Should we be cleaning things up, like telling the Manager?
            println!("AE:{}:-", ae.name);
        });

        (join_handle, ctrl_tx)
    }
}

#[derive(Debug)]
struct AeActor {
    done: bool,
}

impl AeActor {
    pub fn new() -> Self {
        Self { done: false }
    }

    fn done(&self) -> bool {
        self.done
    }
}

impl Actor for AeActor {
    fn process_msg_any(&mut self, _reply_tx: Option<&Sender<BoxMsgAny>>, msg_any: BoxMsgAny) {
        if let Some(msg) = msg_any.downcast_ref::<MsgAeAddActor>() {
            println!("{}.prossess_msg_any: msg={msg:?}", self.name());
        } else if let Some(msg) = msg_any.downcast_ref::<MsgAeDone>() {
            println!("{}.prossess_msg_any: msg={msg:?}", self.name());
            self.done = true;
        } else {
            println!("{}.prossess_msg_any: Uknown msg", self.name());
        }
    }

    fn name(&self) -> &str {
        "AeActor"
    }

    fn done(&self) -> bool {
        self.done()
    }
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgAeAddActor {
    actor: Box<dyn Actor>,
}

#[derive(Debug)]
pub struct MsgAeDone;

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

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

impl Manager {
    pub fn new() -> Manager {
        Self { things: vec![] }
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
                ManagedThing::ItsChannel(t) => Some(t.tx.clone()),
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

                    // Replace the thing with it's channel
                    let mt =
                        std::mem::replace(&mut self.things[handle], ManagedThing::ItsChannel(tc));
                    match mt {
                        ManagedThing::TheThing(thing) => Some(thing),
                        // This should/cannot ever happen!!
                        _ => {
                            panic!("Manager::own_thing: swap returned TheThing but this should never happen");
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
    fn test_non_threaded() {
        println!("\ntest_non_threaded:+");
        let thing = Thing::new("t1", ThingChannel::new());
        println!("test_non_threaded: thing1={thing:?}");
        let mut manager = Manager::new();
        println!("test_non_threaded: new manager={manager:?}");
        let t1_handle = manager.add_thing(thing);
        println!("test_non_threaded: t1_handle={t1_handle} manager={manager:?}");

        let mut t1 = manager.own_thing(t1_handle).unwrap();
        let t1_tx = manager.get_tx_for_thing(t1_handle).unwrap();

        t1_tx.send(Box::new(MsgInc {})).unwrap();
        let msg = t1.channel.rx.recv().unwrap();
        t1.process_msg_any(None, msg);
        println!("test_non_threaded:- t1={t1:?}");
    }

    #[test]
    fn test_executor() {
        println!("\ntest_executor:+");
        let (executor1_join_handle, executor1_tx) = ActorsExecutor::start("executor1");
        println!("test_executor: executor1_tx={executor1_tx:?}");

        let thing = Thing::new("t1", ThingChannel::new());
        println!("test_executor: thing1={thing:?}");
        //let mut manager = Manager::new();
        //println!("main: new manager={manager:?}");
        //let t1_handle = manager.add_thing(thing);
        //println!("main: t1_handle={t1_handle} manager={manager:?}");

        //let mut t1 = manager.own_thing(t1_handle).unwrap();
        //let t1_tx = manager.get_tx_for_thing(t1_handle).unwrap();

        let msg = Box::new(MsgAeAddActor {
            actor: Box::new(thing),
        });
        executor1_tx.send(msg).unwrap();

        let msg = Box::new(MsgAeDone);
        executor1_tx.send(msg).unwrap();

        executor1_join_handle.join().unwrap();

        //t1_tx.send(Box::new(MsgInc{})).unwrap();
        //let msg = t1.channel.rx.recv().unwrap();
        //t1.process_msg_any(None, msg);
        //println!("main: t1={t1:?}");
        println!("test_executor:-");
    }
}
