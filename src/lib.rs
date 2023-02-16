//! Need to add ActorExecutor MsgAddActor -> MsgReplyctor
//! Need to add Manager MsgAddActor -> MsgReplyActor
//! Need to add Manager MsgGetActor -> MsgReplyActor
//! 
//! Need messages:
//!   MsgAddActor { actor_id: ActorInstanceId, their_channel: BiDirLocalChannel }
//!   MsgGetActor { actor_ActorInscneId }
//!   MsgReplyActor { actor_id: ActorInstanceId, their_channel: BiDirLocalChannel }
//! 
//! 
use std::{
    fmt::Debug,
    thread::{self, JoinHandle},
};

use crossbeam_channel::{unbounded, Receiver, Select, Sender};

pub type BoxMsgAny = Box<dyn std::any::Any + Send>;

pub trait Actor: Send + Debug {
    fn process_msg_any(&mut self, reply_tx: Option<&Sender<BoxMsgAny>>, msg: BoxMsgAny);
    fn name(&self) -> &str;
    fn done(&self) -> bool;
    fn get_bi_dir_channel_for_actor(&self, handle: usize) -> Option<BiDirLocalChannel> {
        None
    }
}

pub trait ActorBiDirChannel: Send + Debug {
    fn clone_tx(&self) -> Sender<BoxMsgAny> {
        panic!("ActorBiDirChannel `fn send_self` not implemented");
    }

    fn send_self(&self, _msg: BoxMsgAny) -> Result<(), Box<dyn std::error::Error>> {
        Err("ActorBiDirChannel `fn send_self` not implemented".into())
    }
    fn send(&self, _msg: BoxMsgAny) -> Result<(), Box<dyn std::error::Error>> {
        Err("ActorBiDirChannel `fn send` not implemented".into())
    }

    fn recv(&self) -> Result<BoxMsgAny, Box<dyn std::error::Error>> {
        Err("ActorBiDirChannel `fn recv` not implemented".into())
    }

    fn get_recv(&self) -> &Receiver<BoxMsgAny> {
        panic!("ActorBiDirChannel `fn get_recv` not implemented");
    }
}

#[derive(Debug, Clone)]
pub struct BiDirLocalChannel {
    self_tx: Sender<BoxMsgAny>,
    tx: Sender<BoxMsgAny>,
    rx: Receiver<BoxMsgAny>,
}

//#[allow(unused)]
#[derive(Debug, Clone)]
pub struct BiDirLocalChannels {
    their_channel: BiDirLocalChannel,
    our_channel: BiDirLocalChannel,
}

impl BiDirLocalChannels {
    pub fn new() -> Box<Self> {
        // left_tx -----> right_rx
        let (left_tx, right_rx) = unbounded();

        // left_rx <---- right_tx
        let (right_tx, left_rx) = unbounded();

        Box::new(Self {
            their_channel: BiDirLocalChannel {
                self_tx: right_tx.clone(),
                tx: left_tx.clone(),
                rx: left_rx,
            },
            our_channel: BiDirLocalChannel {
                self_tx: left_tx,
                tx: right_tx,
                rx: right_rx,
            },
        })
    }
}

impl ActorBiDirChannel for BiDirLocalChannel {
    fn clone_tx(&self) -> Sender<BoxMsgAny> {
        self.tx.clone()
    }

    fn get_recv(&self) -> &Receiver<BoxMsgAny> {
        &self.rx
    }

    fn send_self(&self, msg: BoxMsgAny) -> Result<(), Box<dyn std::error::Error>> {
        self.self_tx
            .send(msg)
            .map_err(|err| format!("Error send_self: {err}").into())
    }

    fn send(&self, msg: BoxMsgAny) -> Result<(), Box<dyn std::error::Error>> {
        self.tx
            .send(msg)
            .map_err(|err| format!("Error send: {err}").into())
    }

    fn recv(&self) -> Result<BoxMsgAny, Box<dyn std::error::Error>> {
        self.rx
            .recv()
            .map_err(|err| format!("Error recv: {err}").into())
    }
}

#[allow(unused)]
#[derive(Debug)]
struct ActorsExecutor {
    pub name: String,
    pub actor_vec: Vec<Box<dyn Actor>>,
    pub bi_dir_channels_vec: Vec<Box<BiDirLocalChannels>>,
}

#[allow(unused)]
impl ActorsExecutor {
    // Returns a thread::JoinHandle and a Box<dyn ActorBiDirChannel> which
    // allows messages to be sent and received from the AeActor.
    pub fn start(name: &str) -> (JoinHandle<()>, Box<BiDirLocalChannel>) {
        let ae_actor_bi_dir_channels = BiDirLocalChannels::new();
        let their_bi_dir_channel = Box::new(ae_actor_bi_dir_channels.their_channel.clone());

        let mut ae = Box::new(Self {
            name: name.to_string(),
            actor_vec: Vec::new(),
            bi_dir_channels_vec: Vec::new(),
        });

        let join_handle = thread::spawn(move || {
            println!("AE:{}:+", ae.name);

            // The 0th selector will always be the AeActor
            let ae_actor = Box::new(AeActor::new());
            ae.actor_vec.push(ae_actor);
            ae.bi_dir_channels_vec.push(ae_actor_bi_dir_channels);
            let mut selector = Select::new();
            let oper_idx = selector.recv(&ae.bi_dir_channels_vec[0].our_channel.get_recv());
            assert_eq!(oper_idx, ae.actor_vec.len() - 1);

            let mut done = false;
            while !done {
                println!("AE:{}: TOL", ae.name);
                let oper = selector.select();
                let oper_idx = oper.index();
                let actor = &mut ae.actor_vec[oper_idx];
                let rx = &ae.bi_dir_channels_vec[oper_idx].our_channel.get_recv();
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

        (join_handle, their_bi_dir_channel)
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
    fn process_msg_any(&mut self, reply_tx: Option<&Sender<BoxMsgAny>>, msg_any: BoxMsgAny) {
        if let Some(msg) = msg_any.downcast_ref::<MsgAeAddActor>() {
            println!("{}.prossess_msg_any: msg={msg:?}", self.name());
        } else if let Some(msg) = msg_any.downcast_ref::<MsgAeDone>() {
            println!("{}.prossess_msg_any: msg={msg:?}", self.name());
            self.done = true;
        } else if let Some(msg) = msg_any.downcast_ref::<MsgGetTheirBiDirChannel>() {
            println!("{}.prossess_msg_any: msg={msg:?}", self.name());
            let bdc = self.get_bi_dir_channel_for_actor(msg.handle).unwrap();
            let msg = Box::new(MsgReplyTheirBiDirChannel {
                bi_dir_channel: Box::new(bdc),
            });
            reply_tx.unwrap().send(msg);
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

#[derive(Debug)]
pub struct MsgAeAddActor {
    actor: Box<dyn Actor>,
}

#[derive(Debug)]
pub struct MsgGetTheirBiDirChannel {
    handle: usize,
}

#[derive(Debug)]
pub struct MsgReplyTheirBiDirChannel {
    bi_dir_channel: Box<BiDirLocalChannel>,
}

#[derive(Debug)]
pub struct MsgAeDone;

#[derive(Debug)]
pub enum ManagedActor {
    TheActor(Box<dyn Actor>),
    TheBiDirChannel(Box<BiDirLocalChannel>),
}

#[derive(Debug)]
pub struct Manager {
    actors: Vec<ManagedActor>,
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

impl Manager {
    pub fn new() -> Manager {
        Self { actors: vec![] }
    }

    pub fn add_actor(&mut self, actor: Box<dyn Actor>) -> usize {
        let idx = self.actors.len();
        self.actors.push(ManagedActor::TheActor(actor));

        idx
    }

    pub fn get_bi_dir_channel_for_actor(
        &self,
        handle: usize,
    ) -> Option<Box<dyn ActorBiDirChannel>> {
        if let Some(ma) = self.actors.get(handle) {
            match ma {
                ManagedActor::TheActor(_) => None,
                ManagedActor::TheBiDirChannel(tc) => Some(tc.clone()),
            }
        } else {
            None
        }
    }

    // Need to make thread safe
    pub fn own_actor(&mut self, handle: usize) -> Option<Box<dyn Actor>> {
        if let Some(ma) = self.actors.get(handle) {
            match ma {
                ManagedActor::TheActor(_actor) => {
                    //let tc = ActorBiDirChannel::new();

                    // Replace the thing with it's channel
                    let ma = std::mem::replace(
                        &mut self.actors[handle],
                        ManagedActor::TheBiDirChannel(bdlc),
                    );
                    match ma {
                        ManagedActor::TheActor(actor) => Some(actor),
                        // This should/cannot ever happen!!
                        _ => {
                            panic!("Manager::own_thing: swap returned TheThing but this should never happen");
                        }
                    }
                }
                ManagedActor::OurChannel(_) => {
                    // Already owned
                    None
                }
            }
        } else {
            None
        }
    }
}

//#[cfg(test)]
//mod tests {
//    use super::*;
//
//    #[derive(Debug)]
//    struct MsgInc;
//
//    #[derive(Debug)]
//    struct MsgGetCounter;
//
//    #[derive(Debug)]
//    struct MsgReplyCounter {
//        counter: i32,
//    }
//
//    #[derive(Debug)]
//    pub struct Thing {
//        pub name: String,
//        pub counter: i32,
//    }
//
//    impl Thing {
//        pub fn new(name: &str) -> Self {
//            Self {
//                name: name.to_string(),
//                counter: 0,
//            }
//        }
//
//        pub fn increment(&mut self) {
//            self.counter += 1;
//            println!("Thing::increment: counter={}", self.counter);
//        }
//    }
//
//    impl Actor for Thing {
//        fn process_msg_any(&mut self, reply_tx: Option<&Sender<BoxMsgAny>>, msg: BoxMsgAny) {
//            if msg.downcast_ref::<MsgInc>().is_some() {
//                self.increment()
//            } else if msg.downcast_ref::<MsgGetCounter>().is_some() {
//                let reply_tx = reply_tx.unwrap();
//                reply_tx
//                    .send(Box::new(MsgReplyCounter {
//                        counter: self.counter,
//                    }))
//                    .unwrap();
//            } else {
//                println!("Thing.prossess_msg_any: Uknown msg");
//            }
//        }
//
//        fn name(&self) -> &str {
//            self.name.as_str()
//        }
//
//        fn done(&self) -> bool {
//            false
//        }
//    }
//
//    #[test]
//    fn test_non_threaded() {
//        println!("\ntest_non_threaded:+");
//        let thing = Box::new(Thing::new("t1"));
//        println!("test_non_threaded: thing1={thing:?}");
//        let mut manager = Manager::new();
//        println!("test_non_threaded: new manager={manager:?}");
//        let t1_handle = manager.add_actor(thing);
//        println!("test_non_threaded: t1_handle={t1_handle} manager={manager:?}");
//
//        // Send MsgInc
//        let mut t1 = manager.own_actor(t1_handle).unwrap();
//        let t1_tx = manager.get_tx_for_thing(t1_handle).unwrap();
//        t1_tx.send(Box::new(MsgInc {})).unwrap();
//
//        // Recv MsgInc and process
//        let t1_rx = manager.get_rx_for_thing(t1_handle).unwrap();
//        let msg_any = t1_rx.recv().unwrap();
//        t1.process_msg_any(None, msg_any);
//
//        // Create a second reply channel and process MsgGetCounter and recv MsgReplyCounter
//        let (tx, rx) = unbounded::<BoxMsgAny>();
//        t1.process_msg_any(Some(&tx), Box::new(MsgGetCounter));
//        let msg_any = rx.recv().unwrap();
//        let msg_reply_counter = msg_any.downcast_ref::<MsgReplyCounter>().unwrap();
//        println!("test_non_threaded:- MsgReplyCounter={msg_reply_counter:?}");
//        assert_eq!(msg_reply_counter.counter, 1);
//    }
//
//    #[test]
//    fn test_executor() {
//        println!("\ntest_executor:+");
//        let (executor1_join_handle, executor1_tx) = ActorsExecutor::start("executor1");
//        println!("test_executor: executor1_tx={executor1_tx:?}");
//
//        let thing = Thing::new("t1");
//        println!("test_executor: thing1={thing:?}");
//        //let mut manager = Manager::new();
//        //println!("main: new manager={manager:?}");
//        //let t1_handle = manager.add_thing(thing);
//        //println!("main: t1_handle={t1_handle} manager={manager:?}");
//
//        //let mut t1 = manager.own_thing(t1_handle).unwrap();
//        //let t1_tx = manager.get_tx_for_thing(t1_handle).unwrap();
//
//        let msg = Box::new(MsgAeAddActor {
//            actor: Box::new(thing),
//        });
//        executor1_tx.send(msg).unwrap();
//
//        let msg = Box::new(MsgAeDone);
//        executor1_tx.send(msg).unwrap();
//
//        executor1_join_handle.join().unwrap();
//
//        //t1_tx.send(Box::new(MsgInc{})).unwrap();
//        //let msg = t1.channel.rx.recv().unwrap();
//        //t1.process_msg_any(None, msg);
//        //println!("main: t1={t1:?}");
//        println!("test_executor:-");
//    }
//}
//
