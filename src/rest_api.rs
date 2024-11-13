use rocket::http::Status;
use rocket::State;
use std::sync::Arc;
use tracing::{instrument, warn};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use rocket::form::FromForm;
use rocket::form::Form;
use crate::common::TrackMap;
use crate::webrtc_utils;
use tracing::{info_span, Instrument};

#[derive(FromForm)]
struct Offer<'r> {
    url: &'r str,
    sdp: &'r str,
}

#[post("/sdp", data = "<form>")]
#[instrument(skip(form, tracks_state))]
async fn handle_sdp_offer(form: Form<Offer<'_>>, tracks_state: &State<TrackMap>) -> (Status, String) {
	let org_url = form.url.to_string();
	let mut url = match url::Url::parse(form.url) {
		Ok(url) => url,
		Err(e) => {
			warn!("Error parsing URL {org_url}: {e:?}");
			return (Status::BadRequest, format!("Error parsing URL {org_url}: {e:?}"));
		}
	};
	let contains = tracks_state.inner().read().contains_key(&org_url);
	if !contains {
		let (username, password) = (url.username().to_string(), url.password().unwrap_or_default().to_string());
		url.set_username("").unwrap();
		url.set_password(None).unwrap();
		let stream_settings = crate::common::StreamSettings {
			username,
			password,
			source_url: url,
		};
		match crate::create_tracks(stream_settings).instrument(info_span!("create_tracks", org_url)).await {
			Ok(tracks) => {
				tracks_state.inner().write().insert(org_url.clone(), tracks.clone());
			}
			Err(e) => {
				error!("Could not create track for camera {org_url} video stream due to error: {e:?}");
				// TODO: keep trying periodically
			}
		}
	}
	let cloned_arc = tracks_state.inner().read().get(&org_url).cloned();
	match cloned_arc {
		Some(tracks) => {
			match RTCSessionDescription::offer(form.sdp.to_string()) {
				Ok(offer) => {
					match webrtc_utils::create_answer(offer, Arc::clone(&tracks.0), Arc::clone(&tracks.1)).await {
						Ok(local_desc) => {
							return (Status::Created, local_desc.sdp);
						},
						Err(e) => {
							warn!("Error creating SDP answer: {}", e);
							return (Status::BadRequest, String::from("bad request"));
						}
					}
				},
				Err(e) => {
					warn!("Error parsing SDP offer: {}", e);
					return (Status::BadRequest, String::from("bad request"));
				}
			}
		},
		None => {
			warn!("Could not find track for camera {org_url}");
			return (Status::BadRequest, String::from("bad request"));
		}
	}
}

#[catch(404)]
fn not_found() -> &'static str {
	"Resource was not found."
}

pub fn stage(tracks: TrackMap) -> rocket::fairing::AdHoc {
	rocket::fairing::AdHoc::on_ignite("SDP", |rocket| async {
		rocket
			.manage(tracks)
			.register("/", catchers![not_found])
			.mount("/v0", routes![handle_sdp_offer])
	})
}
