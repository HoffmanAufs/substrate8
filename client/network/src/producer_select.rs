#![allow(missing_docs)]

use crate::{
	config::{self, ProtocolId, Role},
	error,
	service::NetworkService,
	utils::LruHashSet,
	Event, ExHashT, ObservedRole,
};

use codec::{Encode, Decode};
use bytes::Bytes;
use futures::{channel::mpsc, prelude::*};
use libp2p::{multiaddr, PeerId};
use prometheus_endpoint::{register, Counter, PrometheusError, Registry, U64};
use sp_runtime::traits::{Block as BlockT, NumberFor};
use std::{
	borrow::Cow,
	collections::{HashMap, BTreeMap},
	iter,
	num::NonZeroUsize,
	pin::Pin,
	sync::{
		Arc,
	},
	time::{Duration, SystemTime},
};

use sp_consensus::{VoteData, VoteElectionRequest};

const MAX_NOTIFICATION_SIZE: u64 = 16 * 1024 * 1024;
const MAX_KNOWN_NOTIFICATIONS: usize = 10240; // ~300kb per peer + overhead.

struct Metrics {
	propagated_numbers: Counter<U64>,
}

impl Metrics {
	fn register(r: &Registry) -> Result<Self, PrometheusError> {
		Ok(Metrics {
			propagated_numbers: register(
				Counter::new(
					"sync_propaget_numbers",
					"Number of producer vote number propagated to at least one peer",
				)?,
				r,
			)?,
		})
	}
}

pub struct ProducerSelectHandlerPrototype {
	protocol_name: Cow<'static, str>,
}

impl ProducerSelectHandlerPrototype {
	/// Create a new instance.
	pub fn new(protocol_id: ProtocolId) -> Self {
		ProducerSelectHandlerPrototype {
			protocol_name: Cow::from({
				let mut proto = String::new();
				proto.push_str("/");
				proto.push_str(protocol_id.as_ref());
				proto.push_str("/producer-select/1");
				proto
			}),
		}
	}

	/// Returns the configuration of the set to put in the network configuration.
	pub fn set_config(&self) -> config::NonDefaultSetConfig {
		config::NonDefaultSetConfig {
			notifications_protocol: self.protocol_name.clone(),
			fallback_names: Vec::new(),
			max_notification_size: MAX_NOTIFICATION_SIZE,
			set_config: config::SetConfig {
				in_peers: 0,
				out_peers: 0,
				reserved_nodes: Vec::new(),
				non_reserved_mode: config::NonReservedPeerMode::Deny,
			},
		}
	}

	/// Turns the prototype into the actual handler. Returns a controller that allows controlling
	/// the behaviour of the handler while it's running.
	///
	/// Important: the transactions handler is initially disabled and doesn't gossip transactions.
	/// You must call [`TransactionsHandlerController::set_gossip_enabled`] to enable it.
	pub fn build<B: BlockT + 'static, H: ExHashT>(
		self,
		service: Arc<NetworkService<B, H>>,
		metrics_registry: Option<&Registry>,
	) -> error::Result<(ProducerSelectHandler<B, H>, ProducerSelectHandlerController<B>)> {
		let event_stream = service.event_stream("producer-select-handler").boxed();
		let (to_handler, from_controller) = mpsc::unbounded();
		// let gossip_enabled = Arc::new(AtomicBool::new(false));
		// let (vote_notification_tx, vote_notification_rx) = mpsc::unbounded();
		let (local_event_tx, local_event_rx) = mpsc::unbounded();

		let handler = ProducerSelectHandler {
			protocol_name: self.protocol_name,
			service,
			event_stream,
			peers: HashMap::new(),
			from_controller,
			metrics: if let Some(r) = metrics_registry {
				Some(Metrics::register(r)?)
			} else {
				None
			},
			vote_map: BTreeMap::new(),
			vote_recv_config: None,
			pending_response: None,
			vote_notification_tx: None,
			local_event_tx: local_event_tx,
			local_event_rx: local_event_rx,

			// vote_sync_number: None,
			// vote_begin_time: SystemTime::UNIX_EPOCH,
			// vote_open_duration: Duration::new(0,0),
		};

		let controller = ProducerSelectHandlerController { 
			to_handler,
			// vote_notification_rx: Some(vote_notification_rx),
		};

		Ok((handler, controller))
	}
}

pub struct ProducerSelectHandlerController<B: BlockT>{
    to_handler: mpsc::UnboundedSender<ToHandler<B>>,
	// to_handler: mpsc::UnboundedSender<VoteElectionRequest<B>>,
}

impl<B: BlockT> ProducerSelectHandlerController<B>{
	// pub fn propagate_number(&self, n: u64, pending_response: mpsc::UnboundedSender<u64>){
	// 	let _ = self.to_handler.unbounded_send(ToHandler::PropagateNumber(n, pending_response));
	// }

	// pub fn propagate_random(&self, vote_num: u64, parent_id: NumberFor<B>){
	// 	let _ = self.to_handler.unbounded_send(ToHandler::PropagateRandom(vote_num, parent_id));
	// }

	// pub fn prepare_vote(&self, sync_number: NumberFor<B>, duration: Duration){
	// 	let _ = self.to_handler.unbounded_send(ToHandler::PrepareVote(sync_number, duration));
	// }

	// pub fn send_vote(&self, vote_data: VoteData<B>, tx: mpsc::UnboundedSender<Option<usize>>){
	// 	let _ = self.to_handler.unbounded_send(ToHandler::SendVote(vote_data, tx));
	// }

	// pub fn send_election_result(&self){
	// 	let _ = self.to_handler.unbounded_send(ToHandler::SendElectionResult);
	// }

	// pub fn build_vote_stream(&self, tx: mpsc::UnboundedSender<(VoteData<B>, PeerId)>){
	// 	let _ = self.to_handler.unbounded_send(ToHandler::BuildVoteStream(tx));
	// }

	pub fn handle_request(&self, request: VoteElectionRequest<B>){
		match request{
			VoteElectionRequest::PropagateVote(vote_data) => {
				let _ = self.to_handler.unbounded_send(ToHandler::PropagateVote(vote_data));
			}
			VoteElectionRequest::BuildVoteStream(tx) =>{
				let _ = self.to_handler.unbounded_send(ToHandler::BuildVoteStream(tx));
			}
			VoteElectionRequest::ReturnElectionResult =>{
				let _ = self.to_handler.unbounded_send(ToHandler::SendElectionResult);
			}
			// VoteElectionRequest::ConfigVoteRound(sync_id)=>{
			// 	let _ = self.to_handler.unbounded_send(ToHandler::ConfigVoteRound(sync_id));
			// }
		}
		// let _ = self.to_handler.unbounded_send(request);
	}
	// pub fn take_vote_notification_rx(&mut self)->Option<mpsc::UnboundedReceiver<VoteData<B>>>{
	// 	self.vote_notification_rx.take()
	// }
}

enum ToHandler<B: BlockT> {
	PropagateNumber(u64, mpsc::UnboundedSender<u64>),
	PropagateRandom(u64, NumberFor<B>),
	SendVote(VoteData<B>, mpsc::UnboundedSender<Option<usize>>),

	PrepareVote(NumberFor<B>, Duration),
	PropagateVote(VoteData<B>),
	SendElectionResult,
	BuildVoteStream(mpsc::UnboundedSender<(VoteData<B>, PeerId)>),
	// ConfigVoteRound(NumberFor<B>),

	// Request(VoteElectionRequest<B>),
}

/// Handler for transactions. Call [`TransactionsHandler::run`] to start the processing.
pub struct ProducerSelectHandler<B: BlockT + 'static, H: ExHashT> {
	protocol_name: Cow<'static, str>,

	/// Network service to use to send messages and manage peers.
	service: Arc<NetworkService<B, H>>,
	/// Stream of networking events.
	event_stream: Pin<Box<dyn Stream<Item = Event> + Send>>,
	// All connected peers
	peers: HashMap<PeerId, Peer<H>>,
	// transaction_pool: Arc<dyn TransactionPool<H, B>>,
	// local_role: config::Role,
	from_controller: mpsc::UnboundedReceiver<ToHandler<B>>,
	/// Prometheus metrics.
	metrics: Option<Metrics>,

	vote_map: BTreeMap<u64, PeerId>,

	// vote_sync_number: Option<NumberFor<B>>,
	// vote_begin_time: SystemTime,
	// vote_open_duration: Duration,
	vote_recv_config: Option<VoteRecvConfig<B>>,
	pending_response: Option<mpsc::UnboundedSender<Option<usize>>>,
	vote_notification_tx: Option<mpsc::UnboundedSender<(VoteData<B>, PeerId)>>,

	/// 
	local_event_tx: mpsc::UnboundedSender<Event>,
	local_event_rx: mpsc::UnboundedReceiver<Event>,
	// self_stream: Pin<Box<dyn Stream<Item = Event> + Send>>,
}

struct VoteRecvConfig<B: BlockT>{
	sync_number: NumberFor<B>,
	begin_time: SystemTime,
	open_duration: Duration
}

#[derive(Encode, Decode, Debug)]
enum VoteElectionNotification<B: BlockT>{
	Vote(VoteData<B>),
	Election(Vec<(Vec<u8>, u64)>),
}

/// Peer information
#[derive(Debug)]
struct Peer<H: ExHashT> {
	/// Holds a set of transactions known to this peer.
	known_transactions: LruHashSet<H>,
	role: ObservedRole,
}

impl<B: BlockT + 'static, H: ExHashT> ProducerSelectHandler<B, H> {
	/// Turns the [`TransactionsHandler`] into a future that should run forever and not be
	/// interrupted.
	pub async fn run(mut self) {
		loop {
			futures::select! {
				network_event = self.event_stream.next().fuse() => {
					if let Some(network_event) = network_event {
						self.handle_network_event(network_event).await;
					} else {
						// Networking has seemingly closed. Closing as well.
						return;
					}
				},

				// message from local
				self_event = self.local_event_rx.select_next_some() =>{
					self.handle_network_event(self_event).await;
				},

				// message = self.from_controller.select_next_some().fuse() => {
				message = self.from_controller.select_next_some() => {
					match message {
						ToHandler::PropagateNumber(_, _) => {
							// self.pending_response = Some(pending_response);
							// self.propagate_number(n, );
						},
						ToHandler::PropagateRandom(vote_num, parent_id) => {
							self.propagate_number(vote_num, parent_id);
						},
						ToHandler::SendVote(vote_data, pending_response) => {
							self.pending_response = Some(pending_response);
							self.propagate_vote(vote_data);
						},
						ToHandler::SendElectionResult=>{
							self.send_election_result().await;
						},
						ToHandler::PrepareVote(sync_number, duration)=>{
							self.prepare_vote(sync_number, duration);
						}
						ToHandler::BuildVoteStream(tx)=>{
							self.vote_notification_tx = Some(tx);
						},

						// request test
						ToHandler::PropagateVote(vote_data) => {
							// log::info!("propagate vote: {:?}", vote_data);
							// self.pendng_response = Some(pending_response);
							self.propagate_vote(vote_data);
						},
						// ToHandler::ConfigVoteRound(NumberFor<B>) =>{
						// 	log::info!("vote round: {}")
						// }
					}
				},
			}
		}
	}

	async fn handle_network_event(&mut self, event: Event) {
		match event {
			Event::Dht(_) => {},
			Event::SyncConnected { remote } => {
				let addr = iter::once(multiaddr::Protocol::P2p(remote.into()))
					.collect::<multiaddr::Multiaddr>();
				let result = self.service.add_peers_to_reserved_set(
					self.protocol_name.clone(),
					iter::once(addr).collect(),
				);
				if let Err(err) = result {
					log::error!(target: "sync", "Add reserved peer failed: {}", err);
				}
			},
			Event::SyncDisconnected { remote } => {
				let addr = iter::once(multiaddr::Protocol::P2p(remote.into()))
					.collect::<multiaddr::Multiaddr>();
				let result = self.service.remove_peers_from_reserved_set(
					self.protocol_name.clone(),
					iter::once(addr).collect(),
				);
				if let Err(err) = result {
					log::error!(target: "sync", "Removing reserved peer failed: {}", err);
				}
			},

			Event::NotificationStreamOpened { remote, protocol, role, .. }
				if protocol == self.protocol_name =>
			{
				let _was_in = self.peers.insert(
					remote,
					Peer {
						known_transactions: LruHashSet::new(
							NonZeroUsize::new(MAX_KNOWN_NOTIFICATIONS).expect("Constant is nonzero"),
						),
						role,
					},
				);
				debug_assert!(_was_in.is_none());
			}
			Event::NotificationStreamClosed { remote, protocol }
				if protocol == self.protocol_name =>
			{
				let _peer = self.peers.remove(&remote);
				debug_assert!(_peer.is_some());
			}

			Event::NotificationsReceived { remote, messages } => {
				for (protocol, message) in messages {
					if protocol != self.protocol_name {
						continue
					}

					if let Ok(msg) = <VoteElectionNotification<B> as Decode>::decode(&mut message.as_ref()){
						match msg {
							VoteElectionNotification::Vote(vote_data) => {
								log::info!("<<<< vote: {:?} from: {:?}", vote_data, remote);
								self.vote_notification_tx.as_ref().map(|v|{
									let _ = v.unbounded_send((vote_data.clone(), remote));
								});

								// let _ = self.vote_notification_tx.unbounded_send(vote_data.clone());

								// if let Some(vote_recv_config) = &self.vote_recv_config{
								// 	if let Ok(elapsed) = vote_recv_config.begin_time.elapsed(){
								// 		let VoteData{vote_num, sync_id} = vote_data;
								// 		if elapsed > vote_recv_config.open_duration{
								// 			log::info!("<<<< (XX) timeout: {}, {:?} from: {:?}", vote_num, sync_id, remote);
								// 			continue;
								// 		}

								// 		if sync_id != vote_recv_config.sync_number{
								// 			log::info!("<<<< (XX) sync error: accept: {}, recv: {} from: {:?}", 
								// 			vote_recv_config.sync_number, sync_id, remote);
								// 			continue;
								// 		}

								// 		log::info!("<<<< (Ok) valid vote: {}, {:?} from: {:?}", vote_num, sync_id, remote);
								// 		self.vote_map.insert(vote_num, remote.clone());
								// 		// let encode_remote = remote.to_bytes().encode();
								// 		// log::info!("remote encode: {}", encode_remote);
								// 	}
								// }
							},
							VoteElectionNotification::Election(election_value_vec) =>{
								// let election_vec = election_vec_value.iter().map()
								let mut election_result_vec = vec![];
								election_value_vec.iter().for_each(|(peer, vote_num)|{
									if let Ok(peer_id) = PeerId::from_bytes(peer){
										election_result_vec.push((peer_id, vote_num));
									}
								});
								log::info!("<<<< election from remote: {:?} client/network/src/producer_select.rs:350", remote);
								election_result_vec.iter().enumerate().for_each(|(i,p)|{
									println!("{}, {:?} client/network/src/producer_select.rs:352", i, p);
								});

								// let election_result_vec = election_value_vec.iter().filter_map(|(p,n)|PeerId::from_bytes(p))
								let local_peer_id = self.service.local_peer_id();
								let election_peer_vec = election_value_vec.iter()
									.filter_map(|(peer, _n)|PeerId::from_bytes(peer).ok())
									.collect::<Vec<_>>();

								let rank = election_peer_vec.iter().position(|peer|peer==local_peer_id);
								if let Some(mut tx) = self.pending_response.clone(){
									// log::info!("return vote rank: {:?}", rank);
									let _ = tx.send(rank).await;
								}
							},
						}
					}
				}
			},

			// Not our concern.
			Event::NotificationStreamOpened { .. } | Event::NotificationStreamClosed { .. } => {},
		}
	}

	// fn start_vote_round(&mut self, block_number: NumberFor<B>, time_out: Duration){
	// 	self.recv_block_number = Some(block_number);
	// 	self.timeout = Some(time_out); 
	// }
	fn prepare_vote(&mut self, sync_number: NumberFor<B>, duration: Duration){
		// if matches!(self.service.local_role(), Role::Authority){
		log::info!("prepare_vote, Recv Vote for {:?}, client/network/src/producer_select.rs:371", duration);
		self.vote_recv_config = Some(VoteRecvConfig{
			sync_number: sync_number,
			begin_time: SystemTime::now(),
			open_duration: duration
		});
		self.vote_map.clear();
		// }
	}

	async fn send_election_result(&mut self){
		let mut propagated_numbers = 0;
		let peerid_bytes_vec = self.vote_map.iter().map(|(&vote_num, peer)|(peer.to_bytes(), vote_num)).collect::<Vec<_>>();

		let local_peer_id = self.service.local_peer_id();

		if matches!(self.service.local_role(), Role::Authority){
			if let Some(mut tx) = self.pending_response.clone(){
				let rank = self.vote_map.iter().position(|(_, p)|p==local_peer_id);
				// log::info!("return vote rank: {:?}", rank);
				let _ = tx.send(rank).await;
			}
		}

		let to_send = <VoteElectionNotification<B>>::Election(peerid_bytes_vec).encode();
		// let to_send = self.vote_map.iter().map(|(_, v)|v.to_bytes()).collect::<Vec<_>>().encode();

		for (_, who) in self.vote_map.iter(){
			// if who == local_peer_id{
			// 	continue;
			// }
		// for (who, peer) in self.peers.iter_mut() {
			propagated_numbers += 1;

            log::info!(">>>> Election to {:?}, client/network/src/producer_select.rs:446", who);
            self.service.write_notification(
                who.clone(),
                self.protocol_name.clone(),
                to_send.clone(),
            );
		}

		if let Some(ref metriecs) = self.metrics {
			metriecs.propagated_numbers.inc_by(propagated_numbers as _)
		}
	}

	fn propagate_vote(&mut self, vote_data: VoteData<B>){
		let VoteData{vote_num, sync_id} = vote_data;

		// // save the local vote
		// if matches!(self.service.local_role(), Role::Authority){
		// 	// this.event_streams.send(Event::NotificationsReceived { remote, messages });
		// 	if let Some(vote_recv_config) = &self.vote_recv_config{
		// 		if vote_recv_config.sync_number == sync_id{
		// 			let &local_peer_id = self.service.local_peer_id();
		// 			self.vote_map.insert(vote_num, local_peer_id);
		// 		}
		// 	}
		// }

		// propagate vote_data to authority
		let mut propagated_numbers = 0;

		let to_send = VoteElectionNotification::Vote(vote_data).encode();

		for (who, peer) in self.peers.iter_mut() {
			// never send transactions to the light node
			// if matches!(peer.role, ObservedRole::Light) {
			// 	continue
			// }

			if ! (matches!(peer.role, ObservedRole::Authority)) {
				log::info!("{:?} is authority, client/network/src/producer_select.rs:317", peer);
				continue;
			}

			// let to_send = VoteData::<B>::new(vote_num, sync_id.clone());
			propagated_numbers += 1;

            log::info!(">>>> {} to {:?}, client/network/src/producer_select.rs:513", vote_num, who);
            self.service.write_notification(
                who.clone(),
                self.protocol_name.clone(),
                to_send.clone(),
            );
		}

		let local_peer_id = self.service.local_peer_id();
		log::info!(">>>> {} to {:?}, client/network/src/producer_select.rs:522", vote_num, local_peer_id);
		let _ = self.local_event_tx.unbounded_send(
			Event::NotificationsReceived{
				remote: local_peer_id.clone(), 
				messages: vec![(self.protocol_name.clone(), Bytes::from(to_send.clone()))],
			}
		);

		if let Some(ref metriecs) = self.metrics {
			metriecs.propagated_numbers.inc_by(propagated_numbers as _)
		}
	}

	fn propagate_number(&mut self, vote_num: u64, sync_id: NumberFor<B>){
		let mut propagated_numbers = 0;

		for (who, peer) in self.peers.iter_mut() {
			// never send transactions to the light node
			if matches!(peer.role, ObservedRole::Light) {
				continue
			}

			// if matches!(peer.role, ObservedRole::Authority) {
			// 	log::info!("{:?} is authority, client/network/src/producer_select.rs:317", peer);
			// }

			let to_send = VoteData::<B>::new(vote_num, sync_id.clone());
			propagated_numbers += 1;

            log::info!(">>>> {} to {:?}, client/network/src/producer_select.rs:470", vote_num, who);
            self.service.write_notification(
                who.clone(),
                self.protocol_name.clone(),
                to_send.encode(),
            );
		}

		if let Some(ref metriecs) = self.metrics {
			metriecs.propagated_numbers.inc_by(propagated_numbers as _)
		}
	}
}