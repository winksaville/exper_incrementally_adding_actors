//! Need to add ActorExecutor MsgAddActor -> MsgReplyActor
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
    thread::{self, JoinHandle}, collections::HashMap,
};

use crossbeam_channel::{unbounded, Receiver, Select, Sender};

pub type BoxMsgAny = Box<dyn std::any::Any + Send>;

pub trait Actor: Send + Debug {
    fn process_msg_any(&mut self, reply_tx: Option<&Sender<BoxMsgAny>>, msg: BoxMsgAny);
    fn name(&self) -> &str;
    fn done(&self) -> bool;
    fn get_bi_dir_channel_for_actor(&self, _handle: usize) -> Option<BiDirLocalChannel> {
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
    pub their_channel: BiDirLocalChannel,
    pub our_channel: BiDirLocalChannel,
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
    done: bool,
}

#[allow(unused)]
impl ActorsExecutor {
    // Returns a thread::JoinHandle and a Box<dyn ActorBiDirChannel> which
    // allows messages to be sent and received from the AeActor.
    pub fn start(name: &str) -> (JoinHandle<()>, Box<BiDirLocalChannel>) {
        let ae_actor_bi_dir_channels = BiDirLocalChannels::new();
        let their_bi_dir_channel = Box::new(ae_actor_bi_dir_channels.their_channel.clone());

        let mut ae = Box::new(Self {
        //let mut ae = Self {
            name: name.to_string(),
            actor_vec: Vec::new(),
            bi_dir_channels_vec: Vec::new(),
            done: false,
        });

        let join_handle = thread::spawn(move || {
            println!("AE:{}:+", ae.name);

            let mut selector = Select::new();
            let oper_idx = selector.recv(&ae_actor_bi_dir_channels.our_channel.get_recv());
            assert_eq!(oper_idx, 0);

            while !ae.done {
                println!("AE:{}: TOL", ae.name);
                let oper = selector.select();
                let oper_idx = oper.index();

                if oper_idx == 0 {
                    // This messageis for the AE itself
                    let rx = ae_actor_bi_dir_channels.our_channel.get_recv();
                    if let Ok(msg_any) = oper.recv(rx).map_err(|why| {
                        // TODO: What to do on errors in general
                        // Error on our selves, we'll be done
                        println!(
                            "AE:{}: error on recv: {why} `done = true`",
                            ae.name(),
                        );
                        ae.done = true;
                    }) {
                        // This is a message for this ActorExecutor!!!
                        if let Some(msg) = msg_any.downcast_ref::<MsgAeAddActor>() {
                            println!("{}.prossess_msg_any: msg={msg:?}", ae.name());
                        } else if let Some(msg) = msg_any.downcast_ref::<MsgAeDone>() {
                            println!("{}.prossess_msg_any: msg={msg:?}", ae.name());
                            ae.done = true;
                        } else if let Some(msg) = msg_any.downcast_ref::<MsgGetTheirBiDirChannel>() {
                            println!("{}.prossess_msg_any: msg={msg:?}", ae.name());
                            if let Some(bdc) = ae.bi_dir_channels_vec.get(msg.handle) {
                                let their_channel = bdc.their_channel.clone();
                                let msg = Box::new(MsgReplyTheirBiDirChannel {
                                    bi_dir_channel: Box::new(their_channel),
                                });

                                ae.bi_dir_channels_vec[0].our_channel.tx.send(msg).unwrap();
                            } else {
                                // TODO: Add Status field in MsgReplyTheirBiDirChannel
                                println!("{}.prossess_msg_any: MsgGetTheirBiDirChannel bad handle={}", ae.name(), msg.handle);
                            }
                        } else {
                            println!("{}.prossess_msg_any: Uknown msg", ae.name());
                        }
                    }
                } else {
                    // This message for one of the actors running in the AE
                    let actor = &mut ae.actor_vec[oper_idx - 1];
                    let rx = ae.bi_dir_channels_vec[oper_idx - 1].our_channel.get_recv();
                    if let Ok(msg_any) = oper.recv(rx).map_err(|why| {
                        panic!("AE:{}: {} error on recv: {why}", ae.name, actor.name())
                    }) {
                        actor.process_msg_any(None, msg_any);
                        if actor.done() {
                            panic!("AE:{}: {} reported done, what to do?", ae.name, actor.name());
                        }
                    }
                }
            }

            // TODO: Should we be cleaning things up, like telling the Manager?
            println!("AE:{}:-", ae.name);
        });

        (join_handle, their_bi_dir_channel)
    }

    fn name(&self) -> &str {
        // This needs an InstanceId
        "ActorExecutor"
    }
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgAeAddActor {
    actor: Box<dyn Actor>,
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgGetTheirBiDirChannel {
    handle: usize,
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgReplyTheirBiDirChannel {
    bi_dir_channel: Box<BiDirLocalChannel>,
}

#[derive(Debug)]
pub struct MsgAeDone;


#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct InstanceId(usize);

#[derive(Debug)]
pub struct ConnMgr {
    actors: HashMap<InstanceId, Box<BiDirLocalChannel>>,
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgCmAddActor {
    instance_id: InstanceId,
    bdlc: Box<BiDirLocalChannel>, // TODO: Should this be Boxed or not?
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgCmReqBdlc {
    instance_id: InstanceId,
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgCmRspBdlc {
    bdlc: Option<Box<BiDirLocalChannel>>,
}

impl Default for ConnMgr {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnMgr {
    pub fn new() -> ConnMgr {
        Self { actors: HashMap::new() }
    }

    pub fn add_actor(&mut self, instance_id: InstanceId, bdlc: Box<BiDirLocalChannel>) -> usize {
        let idx = self.actors.len();
        self.actors.insert(instance_id, bdlc);

        idx
    }

    pub fn get_actor_bi_dir_channel(
        &self,
        instance_id: InstanceId,
    ) -> Option<Box<BiDirLocalChannel>> {
        if let Some(bdlc) = self.actors.get(&instance_id) {
            Some(bdlc.clone())
        } else {
            None
        }
    }
}

impl Actor for ConnMgr {
    fn process_msg_any(&mut self, rsp_tx: Option<&Sender<BoxMsgAny>>, msg_any: BoxMsgAny) {
        if let Some(msg) = msg_any.downcast_ref::<MsgCmAddActor>() {
            println!("{}.prossess_msg_any: msg={msg:?}", self.name());
            self.add_actor(msg.instance_id.clone(), msg.bdlc.clone());
        } else if let Some(msg) = msg_any.downcast_ref::<MsgCmReqBdlc>() {
            if let Some(tx) = rsp_tx {
                println!("{}.prossess_msg_any: msg={msg:?}", self.name());
                let bdlc =  self.get_actor_bi_dir_channel(msg.instance_id.clone());
                let msg = Box::new(MsgCmRspBdlc { bdlc });
                tx.send(msg).unwrap();
            } else {
                println!("{}.prossess_msg_any: No rsp_tx for MsgCmReqBdlc", self.name());
            }
        } else {
            println!("{}.prossess_msg_any: Uknown msg", self.name());
        }
    }

    fn name(&self) -> &str {
        "ConnMgr"
    }

    fn done(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MsgInc;

    #[derive(Debug)]
    struct MsgGetCounter;

    #[derive(Debug)]
    struct MsgReplyCounter {
        #[allow(unused)]
        counter: i32,
    }

    #[derive(Debug)]
    pub struct Thing {
        pub name: String,
        pub counter: i32,
    }

    impl Thing {
        pub fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                counter: 0,
            }
        }

        pub fn increment(&mut self) {
            self.counter += 1;
            println!("Thing::increment: counter={}", self.counter);
        }
    }

    impl Actor for Thing {
        fn process_msg_any(&mut self, reply_tx: Option<&Sender<BoxMsgAny>>, msg: BoxMsgAny) {
            if msg.downcast_ref::<MsgInc>().is_some() {
                self.increment()
            } else if msg.downcast_ref::<MsgGetCounter>().is_some() {
                let reply_tx = reply_tx.unwrap();
                reply_tx
                    .send(Box::new(MsgReplyCounter {
                        counter: self.counter,
                    }))
                    .unwrap();
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

    #[test]
    fn test_conn_mgr() {
        println!("\ntest_conn_mgr:+");

        // Start an ActorsExecutor
        let (executor1_join_handle, executor1_tx) = ActorsExecutor::start("executor1");
        println!("test_conn_mgr: executor1_tx={executor1_tx:?}");

        // Create Actor Thing
        let thing = Thing::new("t1");
        println!("test_conn_mgr: thing1={thing:?}");

        // Add Thing to the executor
        let msg = Box::new(MsgAeAddActor {
            actor: Box::new(thing),
        });
        executor1_tx.send(msg).unwrap();

        ////t1_tx.send(Box::new(MsgInc{})).unwrap();
        ////let msg = t1.channel.rx.recv().unwrap();
        ////t1.process_msg_any(None, msg);
        ////println!("test_conn_mgr: t1={t1:?}");

        let msg = Box::new(MsgAeDone);
        executor1_tx.send(msg).unwrap();

        executor1_join_handle.join().unwrap();

        println!("test_conn_mgr:-");
    }

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
}
