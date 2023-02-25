use std::{
    cell::UnsafeCell,
    collections::HashMap,
    fmt::Debug,
    thread::{self, JoinHandle},
};

use crossbeam_channel::{unbounded, Receiver, Select, Sender};

pub type BoxMsgAny = Box<dyn std::any::Any + Send>;

/// Receiver References using interior mutability
#[allow(unused)]
#[derive(Debug)]
struct VecBdlcs(UnsafeCell<Vec<BiDirLocalChannels>>);

#[allow(unused)]
impl VecBdlcs {
    pub fn new() -> Self {
        Self(UnsafeCell::new(Vec::new()))
    }

    // Panic's if idx is out of bounds
    pub fn get(&self, idx: usize) -> &BiDirLocalChannels {
        println!("Refs.get({idx}):+");
        let bdlcs = unsafe {
            let v = &*self.0.get();
            &v[idx]
        };
        println!("Refs.get({idx}): bdlcs={bdlcs:?}");
        bdlcs
    }

    pub fn push(&self, v: Box<BiDirLocalChannels>) {
        println!("Refs.push({v:?}):+");
        unsafe {
            let ptr = &mut *self.0.get();
            ptr.push(*v);
        }
        println!("Refs.push():-");
    }

    pub fn len(&self) -> usize {
        println!("Refs.len():+");
        let len = unsafe {
            let v = &*self.0.get();
            v.len()
        };
        println!("Refs.len():- {len}");

        len
    }
}

pub trait Actor: Send + Debug + Sync {
    fn process_msg_any(&mut self, rsp_tx: Option<&Sender<BoxMsgAny>>, msg: BoxMsgAny);
    fn name(&self) -> &str;
    fn done(&self) -> bool;
    fn get_bi_dir_channel_for_actor(&self, _handle: usize) -> Option<BiDirLocalChannel> {
        None
    }
}

pub trait ActorBiDirChannel: Send + Debug {
    fn clone_tx(&self) -> Sender<BoxMsgAny> {
        panic!("ActorBiDirChannel `fn clone_tx` not implemented");
    }

    fn clone_rx(&self) -> Receiver<BoxMsgAny> {
        panic!("ActorBiDirChannel `fn clone_rx` not implemented");
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

    fn clone_rx(&self) -> Receiver<BoxMsgAny> {
        self.rx.clone()
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

    fn get_recv(&self) -> &Receiver<BoxMsgAny> {
        &self.rx
    }
}

#[allow(unused)]
#[derive(Debug)]
struct ActorsExecutor {
    pub name: String,
    pub actor_vec: Vec<Box<dyn Actor>>,
    pub bi_dir_channels_vec: VecBdlcs, //Vec<Box<BiDirLocalChannels>>,
    done: bool,
}

#[allow(unused)]
impl ActorsExecutor {
    // Returns a thread::JoinHandle and a Box<dyn ActorBiDirChannel> which
    // allows messages to be sent and received from the AeActor.
    pub fn start(name: &str) -> (JoinHandle<()>, Box<BiDirLocalChannel>) {
        let ae_actor_bi_dir_channels = BiDirLocalChannels::new();
        let their_bi_dir_channel = Box::new(ae_actor_bi_dir_channels.their_channel.clone());

        // Convert name to string so it can be moved into the thread
        let name = name.to_string();

        let join_handle = thread::spawn(move || {
            let mut ae = Self {
                name: name.to_string(),
                actor_vec: Vec::new(),
                bi_dir_channels_vec: VecBdlcs::new(),
                done: false,
            };
            println!("AE:{}:+", ae.name);

            let mut selector = Select::new();
            let oper_idx = selector.recv(ae_actor_bi_dir_channels.our_channel.get_recv());
            assert_eq!(oper_idx, 0);

            while !ae.done {
                println!("AE:{}: TOL", ae.name);
                let oper = selector.select();
                let oper_idx = oper.index();

                if oper_idx == 0 {
                    // This messageis for the AE itself
                    let rx = ae_actor_bi_dir_channels.our_channel.get_recv();

                    let result = oper.recv(rx);
                    match result {
                        Err(why) => {
                            // TODO: Error on our selves make done, is there anything else we need to do?
                            println!("AE:{}: error on recv: {why} `done = true`", ae.name);
                            ae.done = true;
                        }
                        Ok(msg_any) => {
                            // This is a message for this ActorExecutor!!!
                            println!("{}: msg_any={msg_any:?}", ae.name);
                            if msg_any.downcast_ref::<MsgReqAeAddActor>().is_some() {
                                // It is a MsgReqAeAddActor, now downcast to concrete message so we can push it to actor_vec
                                let msg = msg_any.downcast::<MsgReqAeAddActor>().unwrap();
                                println!("{}: msg={msg:?}", ae.name);
                                let actor_idx = ae.actor_vec.len();
                                ae.actor_vec.push(msg.actor);

                                // Create the bdlcs and add to bi_dir_channels_vec
                                println!("{}: create BiDirLocalChannels", ae.name());
                                let bdlcs = BiDirLocalChannels::new();

                                assert_eq!(ae.bi_dir_channels_vec.len(), actor_idx);

                                ae.bi_dir_channels_vec.push(bdlcs);
                                let bdlcs = ae.bi_dir_channels_vec.get(actor_idx);

                                println!("{}: selector.recv(our_channel.get_recv())", ae.name());
                                selector.recv(bdlcs.our_channel.get_recv());

                                // Send the response message with their_channel
                                let msg_rsp = Box::new(MsgRspAeAddActor {
                                    bdlc: Box::new(bdlcs.their_channel.clone()),
                                });
                                println!("{}: msg.rsp_tx.send msg={msg_rsp:?}", ae.name());
                                msg.rsp_tx.send(msg_rsp);

                                println!(
                                    "{}: added new receiver for {}",
                                    ae.name(),
                                    ae.actor_vec[actor_idx].name()
                                );
                            } else if let Some(msg) = msg_any.downcast_ref::<MsgAeDone>() {
                                println!("{}: msg={msg:?}", ae.name());
                                ae.done = true;
                            } else if let Some(msg) =
                                msg_any.downcast_ref::<MsgReqTheirBiDirChannel>()
                            {
                                println!("{}: msg={msg:?}", ae.name());
                                let bdc = ae.bi_dir_channels_vec.get(msg.handle);
                                let their_channel = bdc.their_channel.clone();
                                let msg_rsp = Box::new(MsgRspTheirBiDirChannel {
                                    bi_dir_channel: Box::new(their_channel),
                                });

                                // send msg_rsp
                                println!("{}: send msg_rsp={msg_rsp:?}", ae.name());
                                msg.rsp_tx.send(msg_rsp).unwrap();
                            } else {
                                println!("{}: Uknown msg", ae.name());
                            }
                        }
                    }
                } else {
                    // This message for one of the actors running in the AE
                    let actor_idx = oper_idx - 1;
                    let actor = &mut ae.actor_vec[actor_idx];
                    println!(
                        "{}: msg for actor_vec[{actor_idx}] {}",
                        ae.name,
                        actor.name()
                    );
                    let bdlcs = ae.bi_dir_channels_vec.get(actor_idx);
                    let rx = bdlcs.our_channel.get_recv();
                    if let Ok(msg_any) = oper.recv(rx).map_err(|why| {
                        // TODO: What should we do here?
                        panic!("{}: {} error on recv: {why}", ae.name, actor.name())
                    }) {
                        actor.process_msg_any(Some(&bdlcs.our_channel.tx), msg_any);
                        if actor.done() {
                            panic!(
                                "AE:{}: {} reported done, what to do?",
                                ae.name,
                                actor.name()
                            );
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
pub struct MsgReqAeAddActor {
    actor: Box<dyn Actor>,
    rsp_tx: Sender<BoxMsgAny>,
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgRspAeAddActor {
    bdlc: Box<BiDirLocalChannel>,
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgReqTheirBiDirChannel {
    handle: usize,
    rsp_tx: Sender<BoxMsgAny>,
}

#[allow(unused)]
#[derive(Debug)]
pub struct MsgRspTheirBiDirChannel {
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
        Self {
            actors: HashMap::new(),
        }
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
        self.actors.get(&instance_id).cloned()
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
                let bdlc = self.get_actor_bi_dir_channel(msg.instance_id.clone());
                let msg = Box::new(MsgCmRspBdlc { bdlc });
                tx.send(msg).unwrap();
            } else {
                println!(
                    "{}.prossess_msg_any: No rsp_tx for MsgCmReqBdlc",
                    self.name()
                );
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

#[inline(never)]
pub fn recv_bdlc(rx: &Receiver<BoxMsgAny>) -> Box<BiDirLocalChannel> {
    println!("recv_bdlc:+");
    let msg_any = rx.recv().unwrap();

    println!("recv_bdlc: msg_any={msg_any:?}");
    let msg = msg_any.downcast_ref::<MsgRspAeAddActor>().unwrap();

    println!("recv_bdlc: msg={msg:?}");

    println!("recv_bdlc:-");
    msg.bdlc.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    //use crossbeam_channel::{unbounded};

    #[derive(Debug)]
    struct MsgReqHello;

    #[derive(Debug)]
    struct MsgRspHello;

    #[derive(Debug)]
    struct MsgInc;

    #[derive(Debug)]
    struct MsgReqCounter;

    #[derive(Debug)]
    struct MsgRspCounter {
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
        fn process_msg_any(&mut self, rsp_tx: Option<&Sender<BoxMsgAny>>, msg: BoxMsgAny) {
            if msg.downcast_ref::<MsgInc>().is_some() {
                println!("{}.process_msg_any: MsgInc", self.name());
                self.increment()
            } else if msg.downcast_ref::<MsgReqCounter>().is_some() {
                println!("{}.process_msg_any: MsgReqCounter", self.name());
                let rsp_tx = rsp_tx.unwrap();
                rsp_tx
                    .send(Box::new(MsgRspCounter {
                        counter: self.counter,
                    }))
                    .unwrap();
                self.increment()
            } else if msg.downcast_ref::<MsgReqHello>().is_some() {
                println!("{}.process_msg_any: MsgReqHello", self.name());
                let rsp_tx = rsp_tx.unwrap();
                rsp_tx.send(Box::new(MsgRspHello)).unwrap();
            } else {
                println!("{}.prossess_msg_any: Uknown msg", self.name());
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
    fn test_msg_req_add_actor() {
        println!("\ntest_msg_req_add_actor:+");
        let (tx, rx) = unbounded::<BoxMsgAny>();

        // Start an ActorsExecutor
        let (executor1_join_handle, executor1_tx) = ActorsExecutor::start("executor1");
        println!("test_msg_req_add_actor: executor1_tx={executor1_tx:?}");

        // Create Actor Thing
        let thing = Thing::new("t1");
        println!("test_msg_req_add_actor: thing1={thing:?}");

        // Add Thing to the executor
        let msg = Box::new(MsgReqAeAddActor {
            actor: Box::new(thing),
            rsp_tx: tx,
        });
        executor1_tx.send(msg).unwrap();
        let thing_bdlc = recv_bdlc(&rx);
        println!("test_msg_req_add_actor: thing_bdlc={thing_bdlc:?}");

        println!("test_msg_req_add_actor: send MsgReqHello");
        thing_bdlc.send(Box::new(MsgReqHello)).unwrap();
        println!("test_msg_req_add_actor: sent MsgReqHello");

        println!("test_msg_req_add_actor: wait MsgRspHello");
        let msg_any = thing_bdlc.recv().unwrap();
        println!("test_msg_req_add_actor: recv MsgRspHello");
        assert!(msg_any.downcast_ref::<MsgRspHello>().is_some());

        println!("test_msg_req_add_actor: send MsgInc");
        thing_bdlc.send(Box::new(MsgInc)).unwrap();
        println!("test_msg_req_add_actor: sent MsgInc");

        println!("test_msg_req_add_actor: send MsgReqCounter");
        thing_bdlc.send(Box::new(MsgReqCounter)).unwrap();
        println!("test_msg_req_add_actor: sent MsgReqCounter");

        println!("test_msg_req_add_actor: recv msg_any of MsgRspCounter");
        let msg_any = thing_bdlc.recv().unwrap();
        println!("test_msg_req_add_actor: recvd {msg_any:?} which should be a MsgRspCounter");

        let msg = msg_any.downcast_ref::<MsgRspCounter>().unwrap();
        println!("test_msg_req_add_actor: msg_any.downcast_ref to {msg:?}");
        assert_eq!(msg.counter, 1);

        println!("test_msg_req_add_actor: send MsgDone");
        let msg = Box::new(MsgAeDone);
        executor1_tx.send(msg).unwrap();
        println!("test_msg_req_add_actor: sent MsgDone");

        println!("test_msg_req_add_actor: join executor1 to complete");
        executor1_join_handle.join().unwrap();
        println!("test_msg_req_add_actor: join executor1 to completed");

        println!("test_msg_req_add_actor:-");
    }
}
