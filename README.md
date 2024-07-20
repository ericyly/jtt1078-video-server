### JT/T 1078交通部标准视频服务说明

- 基于Rust语言、Axum Web框架、Tokio异步运行时实现的1078部标模拟视频服务。
- 视频服务同时也是Web服务，接受Web请求，并给与响应，实现模拟视频服务所有Web功能。
- 视频服务读取rtp文件并缓存，接收websocket视频数据请求，将缓存数据按JT/T 1078音视频包格式，组织为网络数据包，以正常视频帧率，通过websocket按一定时间间隔发送数据包到浏览器，浏览器端直接解码播放。
- 每一路websocket请求，视频服务模拟一个视频终端。
- 本工程为视频服务代码实现，[JTT1078-wasm-player](https://github.com/ericyly/JTT1078-Wasm-Player)工程包括浏览器前端代码实现和本工程编译后可运行程序，包括运行于Windows和Linux操作系统。
