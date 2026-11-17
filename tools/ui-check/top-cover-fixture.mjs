export function installTopCoverFixture() {
  const data = window.__uiFixture, options = data.lightingOptions;
  options.capabilities.topCover = true; options.capabilities.logo = false;
  options.current.logo = null;
  options.current.topCover = { brightness: 3, color: { r: 10, g: 20, b: 30 } };
  data.profile.settings.lightingOptions = { keyboardType: options.capabilities.keyboardType, values: structuredClone(options.current) };
  data.profile.settings.lightingOptions.values.topCover = { brightness: 2, color: { r: 171, g: 205, b: 239 } };
}
