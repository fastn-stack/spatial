//! Shell-Core Protocol
//!
//! The spatial framework uses a shell-core architecture:
//! - **Shell**: Platform-specific (JS/WebXR, Swift/visionOS, Android/Quest)
//!   - Handles platform APIs (WebGL, Metal, etc.)
//!   - Captures input events
//!   - Executes rendering commands
//!   - Manages asset loading, networking, media
//!
//! - **Core**: Platform-agnostic Rust code
//!   - Receives Events from shell
//!   - Maintains scene state machine
//!   - Emits Commands for shell to execute
//!   - No threads, purely event-driven
//!
//! ## Architecture
//!
//! Events and Commands use an enum-of-enums pattern for modularity:
//! - Handlers can subscribe to specific event categories
//! - Modules only see events relevant to them
//! - Better organization and type safety

use serde::{Deserialize, Serialize};

// ============================================================================
// IDs - All IDs are opaque strings (GUIDs, URLs, etc.)
// ============================================================================

/// Unique identifier for volumes/objects in the scene
pub type VolumeId = String;

/// Unique identifier for assets (files being loaded)
pub type AssetId = String;

/// Unique identifier for input devices
pub type DeviceId = String;

/// Unique identifier for network connections (WebRTC, WebSocket)
pub type ConnectionId = String;

/// Unique identifier for media streams/tracks
pub type MediaId = String;

/// Unique identifier for data channels
pub type ChannelId = String;

/// Unique identifier for textures/surfaces
pub type TextureId = String;

/// Unique identifier for timers
pub type TimerId = String;

// ============================================================================
// EVENTS (Shell -> Core)
// ============================================================================

/// Top-level events sent from Shell to Core
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "category", content = "event")]
pub enum Event {
    /// Application lifecycle events
    Lifecycle(LifecycleEvent),
    /// Input device events (keyboard, mouse, touch, gamepad)
    Input(InputEvent),
    /// XR/spatial computing events
    Xr(XrEvent),
    /// Asset loading events
    Asset(AssetEvent),
    /// Scene/volume events
    Scene(SceneEvent),
    /// Network events (WebSocket, WebRTC)
    Network(NetworkEvent),
    /// Media streaming events
    Media(MediaEvent),
    /// Timer events
    Timer(TimerEvent),
}

// ----------------------------------------------------------------------------
// Lifecycle Events
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LifecycleEvent {
    /// Shell initialized, provides capabilities
    Init(InitEvent),
    /// Render frame requested (called every frame)
    Frame(FrameEvent),
    /// Viewport/window resized
    Resize(ResizeEvent),
    /// Application going to background
    Pause,
    /// Application resuming from background
    Resume,
    /// Application shutting down
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitEvent {
    pub platform: Platform,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub dpr: f32,
    pub xr_supported: bool,
    pub xr_immersive_vr: bool,
    pub xr_immersive_ar: bool,
    pub webrtc_supported: bool,
    pub websocket_supported: bool,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    WebGL,
    WebGPU,
    VisionOS,
    Quest,
    Android,
    IOS,
    Desktop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameEvent {
    pub time: f64,
    pub dt: f32,
    pub frame: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeEvent {
    pub width: u32,
    pub height: u32,
    pub dpr: f32,
}

// ----------------------------------------------------------------------------
// Input Events
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    Keyboard(KeyboardEvent),
    Mouse(MouseEvent),
    Touch(TouchEvent),
    Gamepad(GamepadEvent),
}

/// Keyboard events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum KeyboardEvent {
    Connected(KeyboardInfo),
    Disconnected { device_id: DeviceId },
    KeyDown(KeyEventData),
    KeyUp(KeyEventData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardInfo {
    pub device_id: DeviceId,
    pub name: String,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEventData {
    pub device_id: DeviceId,
    pub key: String,
    pub code: String,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
    pub repeat: bool,
}

/// Mouse events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum MouseEvent {
    Connected(MouseInfo),
    Disconnected { device_id: DeviceId },
    Move(MouseMoveData),
    Down(MouseButtonData),
    Up(MouseButtonData),
    Wheel(MouseWheelData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseInfo {
    pub device_id: DeviceId,
    pub name: String,
    pub is_virtual: bool,
    pub has_wheel: bool,
    pub button_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseMoveData {
    pub device_id: DeviceId,
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseButtonData {
    pub device_id: DeviceId,
    pub x: f32,
    pub y: f32,
    pub button: MouseButton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseWheelData {
    pub device_id: DeviceId,
    pub x: f32,
    pub y: f32,
    pub dx: f32,
    pub dy: f32,
}

/// Touch events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum TouchEvent {
    Connected(TouchInfo),
    Disconnected { device_id: DeviceId },
    Start(TouchData),
    Move(TouchData),
    End(TouchData),
    Cancel(TouchData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchInfo {
    pub device_id: DeviceId,
    pub name: String,
    pub is_virtual: bool,
    pub max_touch_points: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchData {
    pub device_id: DeviceId,
    pub touches: Vec<TouchPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchPoint {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub force: Option<f32>,
}

/// Gamepad events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum GamepadEvent {
    Connected(GamepadInfo),
    Disconnected { device_id: DeviceId },
    Input(GamepadInputData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamepadInfo {
    pub device_id: DeviceId,
    pub name: String,
    pub axes_count: u32,
    pub buttons_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamepadInputData {
    pub device_id: DeviceId,
    pub axes: Vec<f32>,
    pub buttons: Vec<(f32, bool)>,
}

// ----------------------------------------------------------------------------
// XR Events
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum XrEvent {
    SessionChanged(XrSessionState),
    HeadPose(PoseData),
    ControllerPose(XrControllerData),
    HandPose(XrHandData),
    Gaze(GazeData),
    Gesture(XrGestureData),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrSessionState {
    None,
    Starting,
    Active,
    Paused,
    Ending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseData {
    pub position: [f32; 3],
    pub orientation: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrControllerData {
    pub hand: Hand,
    pub pose: PoseData,
    pub grip_pose: Option<PoseData>,
    pub buttons: Vec<(f32, bool)>,
    pub axes: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hand {
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrHandData {
    pub hand: Hand,
    pub joints: Vec<PoseData>,
    pub pinch_strength: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GazeData {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrGestureData {
    pub gesture: XrGesture,
    pub hand: Option<Hand>,
    pub position: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrGesture {
    Tap,
    DoubleTap,
    LongPress,
    Pinch,
    Drag,
    Rotate,
    Zoom,
}

// ----------------------------------------------------------------------------
// Asset Events
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AssetEvent {
    LoadStarted { asset_id: AssetId, path: String },
    LoadProgress { asset_id: AssetId, loaded: u64, total: Option<u64> },
    Loaded(AssetLoadedData),
    LoadFailed { asset_id: AssetId, error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetLoadedData {
    pub asset_id: AssetId,
    pub path: String,
    pub asset_type: AssetType,
    pub meshes: Vec<MeshInfo>,
    pub animations: Vec<AnimationInfo>,
    pub skeletons: Vec<SkeletonInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetType {
    Glb,
    Gltf,
    Usd,
    Usdz,
    Obj,
    Image,
    Video,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshInfo {
    pub index: u32,
    pub name: Option<String>,
    pub vertex_count: u32,
    pub has_skeleton: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationInfo {
    pub name: String,
    pub duration_secs: f32,
    pub target_skeleton: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonInfo {
    pub name: String,
    pub bones: Vec<BoneInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneInfo {
    pub index: u32,
    pub name: String,
    pub parent_index: Option<u32>,
}

// ----------------------------------------------------------------------------
// Scene Events
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SceneEvent {
    VolumeReady { volume_id: VolumeId },
    VolumeAnimationComplete { volume_id: VolumeId, animation_id: String },
    TextureReady { texture_id: TextureId },
    TextureError { texture_id: TextureId, error: String },
}

// ----------------------------------------------------------------------------
// Network Events
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NetworkEvent {
    WebSocket(WebSocketEvent),
    Rtc(RtcEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum WebSocketEvent {
    Connected { connection_id: ConnectionId },
    Disconnected { connection_id: ConnectionId, code: u16, reason: String },
    Message { connection_id: ConnectionId, data: DataPayload },
    Error { connection_id: ConnectionId, error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataPayload {
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum RtcEvent {
    ConnectionStateChanged { connection_id: ConnectionId, state: RtcConnectionState },
    IceCandidate { connection_id: ConnectionId, candidate: String, sdp_mid: Option<String>, sdp_mline_index: Option<u16> },
    TrackAdded { connection_id: ConnectionId, track: RtcTrackInfo },
    TrackRemoved { connection_id: ConnectionId, media_id: MediaId },
    DataChannelOpened { connection_id: ConnectionId, channel_id: ChannelId, label: String },
    DataChannelClosed { connection_id: ConnectionId, channel_id: ChannelId },
    DataChannelMessage { connection_id: ConnectionId, channel_id: ChannelId, data: DataPayload },
    DataChannelError { connection_id: ConnectionId, channel_id: ChannelId, error: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RtcConnectionState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtcTrackInfo {
    pub media_id: MediaId,
    pub kind: MediaKind,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Audio,
    Video,
}

// ----------------------------------------------------------------------------
// Media Events
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MediaEvent {
    StreamReady { media_id: MediaId, tracks: Vec<MediaTrackInfo> },
    StreamEnded { media_id: MediaId },
    FrameAvailable { media_id: MediaId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTrackInfo {
    pub track_id: String,
    pub kind: MediaKind,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

// ----------------------------------------------------------------------------
// Timer Events
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TimerEvent {
    Fired { timer_id: TimerId },
}

// ============================================================================
// COMMANDS (Core -> Shell)
// ============================================================================

/// Top-level commands sent from Core to Shell
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "category", content = "command")]
pub enum Command {
    /// Asset management commands
    Asset(AssetCommand),
    /// Scene/volume commands
    Scene(SceneCommand),
    /// Animation and rig commands
    Animation(AnimationCommand),
    /// Material and texture commands
    Material(MaterialCommand),
    /// Environment commands (camera, background, lighting)
    Environment(EnvironmentCommand),
    /// Timer commands
    Timer(TimerCommand),
    /// XR commands
    Xr(XrCommand),
    /// Network commands
    Network(NetworkCommand),
    /// Media commands
    Media(MediaCommand),
    /// Debug/logging commands
    Debug(DebugCommand),
}

// ----------------------------------------------------------------------------
// Asset Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum AssetCommand {
    Load { asset_id: AssetId, path: String },
    Cancel { asset_id: AssetId },
    Unload { asset_id: AssetId },
}

// ----------------------------------------------------------------------------
// Scene Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum SceneCommand {
    CreateVolume(CreateVolumeData),
    DestroyVolume { volume_id: VolumeId },
    SetTransform(SetTransformData),
    SetVisible { volume_id: VolumeId, visible: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVolumeData {
    pub volume_id: VolumeId,
    pub source: VolumeSource,
    pub transform: Transform,
    pub material: Option<MaterialOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VolumeSource {
    Primitive(Primitive),
    Asset { asset_id: AssetId, mesh_index: Option<u32> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Primitive {
    Cube { size: f32 },
    Box { width: f32, height: f32, depth: f32 },
    Sphere { radius: f32, segments: u32 },
    Cylinder { radius: f32, height: f32, segments: u32 },
    Plane { width: f32, height: f32 },
    Quad { width: f32, height: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTransformData {
    pub volume_id: VolumeId,
    pub transform: Transform,
    pub animate: Option<AnimateTransform>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateTransform {
    pub duration_ms: u32,
    pub easing: Easing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(u32),
}

// ----------------------------------------------------------------------------
// Animation Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum AnimationCommand {
    Play(PlayAnimationData),
    Stop { volume_id: VolumeId, animation_id: Option<String> },
    SetBoneTransform(SetBoneTransformData),
    SetBoneTransforms(SetBoneTransformsData),
    SetBlendShape(SetBlendShapeData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayAnimationData {
    pub volume_id: VolumeId,
    pub animation_id: String,
    pub animation_name: String,
    pub speed: f32,
    pub loop_mode: LoopMode,
    pub weight: f32,
    pub start_time: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    Once,
    Loop,
    PingPong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBoneTransformData {
    pub volume_id: VolumeId,
    pub bone_name: String,
    pub transform: BoneTransform,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBoneTransformsData {
    pub volume_id: VolumeId,
    pub bones: Vec<(String, BoneTransform, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoneTransform {
    pub position: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBlendShapeData {
    pub volume_id: VolumeId,
    pub blend_shape_name: String,
    pub weight: f32,
}

// ----------------------------------------------------------------------------
// Material Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum MaterialCommand {
    SetMaterial(SetMaterialData),
    CreateTexture(CreateTextureData),
    UpdateTexture(UpdateTextureData),
    DestroyTexture { texture_id: TextureId },
    BindMediaToTexture { texture_id: TextureId, media_id: MediaId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialOverride {
    pub color: Option<[f32; 4]>,
    pub texture_id: Option<TextureId>,
    pub metallic: Option<f32>,
    pub roughness: Option<f32>,
    pub emissive: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetMaterialData {
    pub volume_id: VolumeId,
    pub slot: Option<u32>,
    pub material: MaterialOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTextureData {
    pub texture_id: TextureId,
    pub source: TextureSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextureSource {
    Asset { asset_id: AssetId },
    Empty { width: u32, height: u32, format: TextureFormat },
    Media { media_id: MediaId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureFormat {
    Rgba8,
    Rgb8,
    R8,
    Rgba16Float,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTextureData {
    pub texture_id: TextureId,
    pub data: TextureData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextureData {
    Pixels { width: u32, height: u32, format: TextureFormat, data: Vec<u8> },
    Svg { svg: String, width: u32, height: u32 },
    Html { html: String, width: u32, height: u32 },
}

// ----------------------------------------------------------------------------
// Environment Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum EnvironmentCommand {
    SetCamera(CameraData),
    SetBackground(BackgroundData),
    SetLighting(LightingData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraData {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fov_degrees: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackgroundData {
    Color([f32; 4]),
    Skybox { asset_id: AssetId },
    Transparent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightingData {
    pub ambient: [f32; 3],
    pub directional: Option<DirectionalLight>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionalLight {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

// ----------------------------------------------------------------------------
// Timer Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum TimerCommand {
    Set { timer_id: TimerId, delay_ms: u32, repeat: bool },
    Cancel { timer_id: TimerId },
}

// ----------------------------------------------------------------------------
// XR Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum XrCommand {
    Enter { mode: XrMode },
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrMode {
    ImmersiveVr,
    ImmersiveAr,
}

// ----------------------------------------------------------------------------
// Network Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NetworkCommand {
    WebSocket(WebSocketCommand),
    Rtc(RtcCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum WebSocketCommand {
    Connect { connection_id: ConnectionId, url: String, protocols: Vec<String> },
    Send { connection_id: ConnectionId, data: DataPayload },
    Close { connection_id: ConnectionId, code: Option<u16>, reason: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum RtcCommand {
    CreateConnection { connection_id: ConnectionId, config: RtcConfiguration },
    CloseConnection { connection_id: ConnectionId },
    CreateOffer { connection_id: ConnectionId },
    CreateAnswer { connection_id: ConnectionId },
    SetLocalDescription { connection_id: ConnectionId, sdp_type: SdpType, sdp: String },
    SetRemoteDescription { connection_id: ConnectionId, sdp_type: SdpType, sdp: String },
    AddIceCandidate { connection_id: ConnectionId, candidate: String, sdp_mid: Option<String>, sdp_mline_index: Option<u16> },
    CreateDataChannel { connection_id: ConnectionId, channel_id: ChannelId, label: String, config: DataChannelConfig },
    SendData { connection_id: ConnectionId, channel_id: ChannelId, data: DataPayload },
    CloseDataChannel { connection_id: ConnectionId, channel_id: ChannelId },
    AddTrack { connection_id: ConnectionId, media_id: MediaId },
    RemoveTrack { connection_id: ConnectionId, media_id: MediaId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtcConfiguration {
    pub ice_servers: Vec<IceServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SdpType {
    Offer,
    Answer,
    Pranswer,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataChannelConfig {
    pub ordered: bool,
    pub max_retransmits: Option<u16>,
    pub max_packet_life_time: Option<u16>,
}

impl Default for DataChannelConfig {
    fn default() -> Self {
        Self {
            ordered: true,
            max_retransmits: None,
            max_packet_life_time: None,
        }
    }
}

// ----------------------------------------------------------------------------
// Media Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum MediaCommand {
    CreateStream { media_id: MediaId, source: MediaSource },
    DestroyStream { media_id: MediaId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaSource {
    Camera { facing: CameraFacing, width: Option<u32>, height: Option<u32> },
    Microphone,
    VideoFile { asset_id: AssetId },
    AudioFile { asset_id: AssetId },
    ScreenCapture,
    Canvas { width: u32, height: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraFacing {
    Front,
    Back,
    Environment,
}

// ----------------------------------------------------------------------------
// Debug Commands
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum DebugCommand {
    Log { level: LogLevel, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

// ============================================================================
// CORE TRAIT
// ============================================================================

/// Trait that the application implements
pub trait Core {
    /// Handle an event from the shell
    /// Returns commands for the shell to execute
    fn handle(&mut self, event: Event) -> Vec<Command>;
}

// ============================================================================
// HELPER TRAITS FOR MODULAR HANDLERS
// ============================================================================

/// Handler for lifecycle events
pub trait LifecycleHandler {
    fn handle_lifecycle(&mut self, event: LifecycleEvent) -> Vec<Command>;
}

/// Handler for input events
pub trait InputHandler {
    fn handle_input(&mut self, event: InputEvent) -> Vec<Command>;
}

/// Handler for XR events
pub trait XrHandler {
    fn handle_xr(&mut self, event: XrEvent) -> Vec<Command>;
}

/// Handler for asset events
pub trait AssetHandler {
    fn handle_asset(&mut self, event: AssetEvent) -> Vec<Command>;
}

/// Handler for scene events
pub trait SceneHandler {
    fn handle_scene(&mut self, event: SceneEvent) -> Vec<Command>;
}

/// Handler for network events
pub trait NetworkHandler {
    fn handle_network(&mut self, event: NetworkEvent) -> Vec<Command>;
}

/// Handler for media events
pub trait MediaHandler {
    fn handle_media(&mut self, event: MediaEvent) -> Vec<Command>;
}

/// Handler for timer events
pub trait TimerHandler {
    fn handle_timer(&mut self, event: TimerEvent) -> Vec<Command>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_loaded_json() {
        let json = r#"{"category":"Asset","event":{"type":"Loaded","asset_id":"asset-1","path":"cube.glb","asset_type":"Glb","meshes":[{"index":0,"name":"default","vertex_count":36,"has_skeleton":false}],"animations":[],"skeletons":[]}}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        match event {
            Event::Asset(AssetEvent::Loaded(data)) => {
                assert_eq!(data.asset_id, "asset-1");
                assert_eq!(data.path, "cube.glb");
            }
            _ => panic!("Expected Asset::Loaded event"),
        }
    }

    #[test]
    fn test_lifecycle_init_json() {
        let json = r#"{"category":"Lifecycle","event":{"type":"Init","platform":"Desktop","viewport_width":1280,"viewport_height":720,"dpr":1.0,"xr_supported":false,"xr_immersive_vr":false,"xr_immersive_ar":false,"webrtc_supported":false,"websocket_supported":false,"features":[]}}"#;
        let event: Event = serde_json::from_str(json).unwrap();
        match event {
            Event::Lifecycle(LifecycleEvent::Init(data)) => {
                assert_eq!(data.viewport_width, 1280);
            }
            _ => panic!("Expected Lifecycle::Init event"),
        }
    }
}
