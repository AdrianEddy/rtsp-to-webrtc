# RTSP To WebRTC

Converts an RTSP stream from an IP camera to a WebRTC stream.

It allows to provide an arbitrary rtsp url and it immediately responds with WebRTC stream. It is designed to not require any config. Simply send the WebRTC SDP offer and RTSP url in a simple POST request (see [example.html](https://github.com/AdrianEddy/rtsp-to-webrtc/blob/main/example.html)) and that's it

This repository builds the binaries for most major systems and architectures on GitHub actions. You can download binaries directly from the [Actions](https://github.com/AdrianEddy/rtsp-to-webrtc/actions) page

## Usage

* Download the binary from the [Actions](https://github.com/AdrianEddy/rtsp-to-webrtc/actions) page or build yourself
	* You can build the docker container, or build locally with `cargo build --release`. Building in release mode is HIGHLY recommended, as performance is MUCH worse under debug mode.
* Run the executable
	* E.g. `./rtsp-to-webrtc`
* If using containers, forward port 8000
* Open `example.html` in your browser on the same machine

If you want to customize the address or port, you can use `Rocket.toml` config file or environment variables. Refer to the [Rocket docs](https://rocket.rs/guide/v0.5/configuration/)



## License

#### This is a fork of [ClusterVMS/rtsp-to-webrtc](https://github.com/ClusterVMS/rtsp-to-webrtc). The main difference is that this fork has removed the configuration files and added support for audio streams

Licensed under either of the Apache License, Version 2.0 or the MIT License at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

