//! Port of `pkg/analyzer/lib/src/manifest/manifest_values.dart`.

pub const ACTIVITY_TAG: &str = "activity";

pub const ANDROID_NAME: &str = "android:name";

pub const ANDROID_PERMISSION_CAMERA: &str = "android.permission.CAMERA";

pub const ANDROID_REQUIRED: &str = "android:required";

pub const APPLICATION_TAG: &str = "application";

/// The Android `resizeableActivity` attribute.
///
/// The manifest parser lowercases attribute names.
pub const ATTRIBUTE_RESIZABLE_ACTIVITY: &str = "android:resizeableactivity";

/// The Android `screenOrientation` attribute.
///
/// The manifest parser lowercases attribute names.
pub const ATTRIBUTE_SCREEN_ORIENTATION: &str = "android:screenorientation";

pub const HARDWARE_FEATURE_CAMERA: &str = "android.hardware.camera";

pub const HARDWARE_FEATURE_CAMERA_AUTOFOCUS: &str = "android.hardware.camera.autofocus";

pub const HARDWARE_FEATURE_TELEPHONY: &str = "android.hardware.telephony";

pub const HARDWARE_FEATURE_TOUCHSCREEN: &str = "android.hardware.touchscreen";

pub const MANIFEST_TAG: &str = "manifest";

pub const UNSUPPORTED_HARDWARE_FEATURES: &[&str] = &[
    HARDWARE_FEATURE_CAMERA,
    HARDWARE_FEATURE_CAMERA_AUTOFOCUS,
    "android.hardware.camera.capability.manual_post_processing",
    "android.hardware.camera.capability.manual_sensor",
    "android.hardware.camera.capability.raw",
    "android.hardware.camera.flash",
    "android.hardware.camera.level.full",
    "android.hardware.consumerir",
    "android.hardware.location.gps",
    "android.hardware.nfc",
    "android.hardware.nfc.hce",
    "android.hardware.sensor.barometer",
    HARDWARE_FEATURE_TELEPHONY,
    "android.hardware.telephony.cdma",
    "android.hardware.telephony.gsm",
    "android.hardware.type.automotive",
    "android.hardware.type.television",
    "android.hardware.usb.accessory",
    "android.hardware.usb.host",
    // Partially supported, only on some Chrome OS devices.
    "android.hardware.sensor.accelerometer",
    "android.hardware.sensor.compass",
    "android.hardware.sensor.gyroscope",
    "android.hardware.sensor.light",
    "android.hardware.sensor.proximity",
    "android.hardware.sensor.stepcounter",
    "android.hardware.sensor.stepdetector",
    // Software features that are not supported.
    "android.software.app_widgets",
    "android.software.device_admin",
    "android.software.home_screen",
    "android.software.input_methods",
    "android.software.leanback",
    "android.software.live_wallpaper",
    "android.software.live_tv",
    "android.software.managed_users",
    "android.software.midi",
    "android.software.sip",
    "android.software.sip.voip",
];

pub const UNSUPPORTED_ORIENTATIONS: &[&str] = &[
    "landscape",
    "portrait",
    "reverseLandscape",
    "reversePortrait",
    "sensorLandscape",
    "sensorPortrait",
    "userLandscape",
    "userPortrait",
];

pub const USES_FEATURE_TAG: &str = "uses-feature";

pub const USES_PERMISSION_TAG: &str = "uses-permission";

pub fn get_implied_unsupported_hardware(permission: Option<&str>) -> Option<&'static str> {
    match permission {
        Some(ANDROID_PERMISSION_CAMERA) => Some(HARDWARE_FEATURE_CAMERA),
        Some(
            "android.permission.CALL_PHONE"
            | "android.permission.CALL_PRIVILEGED"
            | "android.permission.MODIFY_PHONE_STATE"
            | "android.permission.PROCESS_OUTGOING_CALLS"
            | "android.permission.READ_SMS"
            | "android.permission.RECEIVE_SMS"
            | "android.permission.RECEIVE_MMS"
            | "android.permission.RECEIVE_WAP_PUSH"
            | "android.permission.SEND_SMS"
            | "android.permission.WRITE_APN_SETTINGS"
            | "android.permission.WRITE_SMS",
        ) => Some(HARDWARE_FEATURE_TELEPHONY),
        _ => None,
    }
}
