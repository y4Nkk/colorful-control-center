export function installMicrophoneFixture() {
  const data = window.__uiFixture;
  data.microphones = { revision: "mic-0", current: { endpoints: [
    { id: "mic-A", name: "内置麦克风", muted: false }, { id: "mic-B", name: "USB麦克风", muted: true },
  ], saved: 0 }, blockedReason: null };
  data.profile.settings.microphones = { endpoints: structuredClone(data.microphones.current.endpoints), savedMuted: true };
  data.profile.settings.microphones.endpoints[0].muted = true;
  data.profile.settings.microphones.endpoints[1].muted = false;
}
