use std::collections::HashMap;
use std::sync::Arc;
use url::Url;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;

pub type TrackMap = parking_lot::RwLock<HashMap<String, (Arc<TrackLocalStaticRTP>, Arc<TrackLocalStaticRTP>)>>;

#[derive(serde::Deserialize, Debug)]
pub struct StreamSettings {
	pub source_url: Url,
	pub username: String,
	pub password: String,
}
