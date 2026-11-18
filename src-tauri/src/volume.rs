//! Windows master volume (default output device) for the volume slider when
//! music plays in another app (Spotify, Apple Music, browser...).

use windows::core::GUID;
use windows::Win32::Foundation::BOOL;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

fn with_endpoint<T: Send + 'static>(
    f: impl FnOnce(&IAudioEndpointVolume) -> windows::core::Result<T> + Send + 'static,
) -> Result<T, String> {
    std::thread::spawn(move || -> Result<T, String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            let dev = en.GetDefaultAudioEndpoint(eRender, eConsole).map_err(|e| e.to_string())?;
            let ep: IAudioEndpointVolume = dev.Activate(CLSCTX_ALL, None).map_err(|e| e.to_string())?;
            f(&ep).map_err(|e| e.to_string())
        }
    })
    .join()
    .map_err(|_| "volume thread panicked".to_string())?
}

pub fn get() -> Result<(f32, bool), String> {
    with_endpoint(|ep| unsafe { Ok((ep.GetMasterVolumeLevelScalar()?, ep.GetMute()?.as_bool())) })
}

pub fn set(v: f32) -> Result<(), String> {
    with_endpoint(move |ep| unsafe {
        ep.SetMasterVolumeLevelScalar(v.clamp(0.0, 1.0), std::ptr::null::<GUID>())?;
        ep.SetMute(BOOL::from(false), std::ptr::null::<GUID>())
    })
}

pub fn mute(on: bool) -> Result<(), String> {
    with_endpoint(move |ep| unsafe { ep.SetMute(BOOL::from(on), std::ptr::null::<GUID>()) })
}
