use clap::{Arg, Command};
use core::time::Duration;
use futures::StreamExt;
use retina::client::PacketItem;
use rocket::{Request, Response};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{error, info_span, Instrument};
use webrtc::api::media_engine::*;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::TrackLocalWriter;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;

#[macro_use] extern crate rocket;

mod common;
mod rest_api;
mod webrtc_utils;

use crate::common::TrackMap;

// Since the UI is served by another server, we may need to setup CORS to allow the UI to make requests to this server.
pub struct CORS;

#[rocket::async_trait]
impl Fairing for CORS {
	fn info(&self) -> Info {
		Info {
			name: "Add CORS headers to responses",
			kind: Kind::Response
		}
	}

	async fn on_response<'r>(&self, _request: &'r Request<'_>, response: &mut Response<'r>) {
		response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
		response.set_header(Header::new("Access-Control-Allow-Methods", "POST, GET, PATCH, OPTIONS"));
		response.set_header(Header::new("Access-Control-Allow-Headers", "*"));
		response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
	}
}



// Originally copied from https://github.com/webrtc-rs/webrtc/tree/master/examples/examples/rtp-to-webrtc
#[rocket::main]
async fn main() -> anyhow::Result<()> {
	let mut app = Command::new("rtsp-to-webrtc")
		.version("0.2.4")
		.author("Alicrow")
		.about("Forwards an RTSP stream as a WebRTC stream.")
		.arg(
			Arg::new("FULLHELP")
				.help("Prints more detailed help information")
				.long("fullhelp"),
		)
		.arg(
			Arg::new("debug")
				.long("debug")
				.short('d')
				.help("Prints debug log information"),
		);

	let matches = app.clone().get_matches();

	if matches.contains_id("FULLHELP") {
		app.print_long_help().unwrap();
		std::process::exit(0);
	}

	let log_level = if matches.contains_id("debug") {
		tracing::Level::DEBUG
	} else {
		tracing::Level::INFO
	};

	tracing_subscriber::fmt()
		.with_max_level(log_level)
		.init();

	let tracks = TrackMap::new(HashMap::new());

	let _rocket = rocket::build()
		.attach(rest_api::stage(tracks))
		.attach(CORS)
		.launch()
		.await?;

	anyhow::Ok(())
}

pub async fn create_tracks(stream_settings: common::StreamSettings) -> anyhow::Result<(Arc<TrackLocalStaticRTP>, Arc<TrackLocalStaticRTP>)> {
	// Create Track that we send video back to client on
	let video_track = Arc::new(TrackLocalStaticRTP::new(
		RTCRtpCodecCapability {
			mime_type: MIME_TYPE_H264.to_owned(),
			..Default::default()
		},
		"video".to_owned(),
		"webrtc-rs".to_owned(),
	));
	let audio_track = Arc::new(TrackLocalStaticRTP::new(
		RTCRtpCodecCapability {
			mime_type: MIME_TYPE_PCMA.to_owned(),
			..Default::default()
		},
		"audio".to_owned(),
		"webrtc-rs".to_owned(),
	));

	// Set up RTSP connection to camera

	let video_track_clone = video_track.clone();
	let audio_track_clone = audio_track.clone();

	// Thread that reads from the input stream and writes packets to the output streams
	tokio::spawn(async move {
		loop {
			let mut video_i = None;
			let mut audio_i = None;
			let session_options = retina::client::SessionOptions::default().creds(Some(retina::client::Credentials {username: stream_settings.username.clone(), password: stream_settings.password.clone()}) );
			let session = match retina::client::Session::describe(stream_settings.source_url.clone(), session_options).await {
				Ok(mut session) => {
					video_i = Some(
						session.streams().iter()
						.position(|s| s.media() == "video" && s.encoding_name() == "h264")
						.ok_or_else(|| error!("Could not find H.264 video stream")).unwrap()
					);
					audio_i = Some(
						session.streams().iter()
						.position(|s| s.media() == "audio" && s.encoding_name() == "pcma")
						.ok_or_else(|| error!("Could not find PCM audio stream")).unwrap()
					);
					match session.setup(video_i.unwrap(), retina::client::SetupOptions::default()).await {
						Ok(_) => {
							match session.setup(audio_i.unwrap(), retina::client::SetupOptions::default()).await {
								Ok(_) => { }
								Err(e) => {
									error!("Failed to setup audio track: {audio_i:?}: {e:?}");
								}
							}
							session.play(retina::client::PlayOptions::default()).await
						}
						Err(e) => Err(e)
					}
				}
				Err(e) => Err(e)
			};

			match session {
				Ok(mut session) => {
					// Read RTP packets forever and send them to the WebRTC Client
					'read_loop: loop {
						match Pin::new(&mut session).next().await {
							None => {
								error!("Source RTSP stream returned None; The stream must have closed.");
								break 'read_loop;
							}
							Some(Err(e)) => {
								error!("error while reading input stream: {e}");
								// FIXME: keep track of whether we're connected or not
								break 'read_loop;
							}
							Some(Ok(PacketItem::Rtp(packet))) => {
								let raw_rtp = packet.raw();
								if packet.stream_id() == video_i.unwrap() {
									if let Err(err) = video_track_clone.write(&raw_rtp).await {
										if webrtc::Error::ErrClosedPipe == err {
											// The peerConnection has been closed.
											// FIXME: when would this even occur?
										} else {
											error!("video_track write err: {}", err);
										}
									}
								} else if packet.stream_id() == audio_i.unwrap() {
									if let Err(err) = audio_track_clone.write(&raw_rtp).await {
										if webrtc::Error::ErrClosedPipe == err {
											// The peerConnection has been closed.
											// FIXME: when would this even occur?
										} else {
											error!("audio_track write err: {}", err);
										}
									}
								} else {
									error!("Unknown stream ID: {}", packet.stream_id());
								}
							}
							Some(Ok(PacketItem::Rtcp(_))) => {
								// Do nothing with RTCP packets for now
							}
							Some(Ok(something)) => {
								error!("Received something that we can't handle; it was {:?}", something);
							}
						}
					}
				}
				Err(e) => {
					error!("Failed to connect to input stream {}, error: {e}", stream_settings.source_url);
				}
			}

			// Sleep for a bit after getting disconnected or failing to connect.
			// If the issue persists, we don't want to waste all our time constantly trying to reconnect.
			tokio::time::sleep(Duration::from_secs(1)).await;
		}
	}.instrument(info_span!("read_loop")));

	Ok((video_track, audio_track))
}
