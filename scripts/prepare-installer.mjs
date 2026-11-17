import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';

const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const literal = value => `@"${value.replaceAll('"', '""')}"`;

export function prepareInstaller(engine, applicationPayload, directory) {
  const config = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
  const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
  if (!/^\d+\.\d+\.\d+$/.test(config.version) || config.version !== pkg.version)
    throw new Error('Installer package and Tauri versions must match and use three numeric components.');
  const source = readFileSync('src/styles/tokens.css', 'utf8');
  const colors = {
    Canvas: 'canvas', Surface: 'content', MutedSurface: 'content-muted', Ink: 'text',
    Muted: 'text-muted', Hairline: 'hairline', Action: 'action', OnAction: 'text-on-action', Error: 'danger',
  };
  const brushes = Object.entries(colors).map(([name, token]) => {
    const value = source.match(new RegExp(`--ccc-${token}:\\s*([^;]+);`))?.[1];
    return `  <SolidColorBrush x:Key="${name}" Color="${wpfColor(value)}" />`;
  });
  const metrics = Object.entries({ PanelRadius: ['radius-panel', 'px'], MotionDistance: ['space-4', 'px'],
    MotionMilliseconds: ['motion', 'ms'], FastMotionMilliseconds: ['motion-fast', 'ms'] }).map(([name, [token, unit]]) => {
    const value = source.match(new RegExp(`--ccc-${token}:\\s*(\\d+)${unit};`))?.[1];
    if (!value) throw new Error(`Missing installer metric: ${token}`);
    return `  <sys:Double x:Key="${name}">${value}</sys:Double>`;
  });
  mkdirSync(directory, { recursive: true });
  writeFileSync(join(directory, 'Palette.xaml'), `<ResourceDictionary xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
 xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" xmlns:sys="clr-namespace:System;assembly=mscorlib">\n${brushes.join('\n')}\n${metrics.join('\n')}\n</ResourceDictionary>\n`);
  const fields = {
    ProductName: config.productName, Version: config.version, ApplicationFileStem: pkg.name,
    EngineSha256: hash(engine), ApplicationSha256: hash(applicationPayload),
  };
  const constants = Object.entries(fields).map(([name, value]) => `        internal const string ${name} = ${literal(value)};`);
  writeFileSync(join(directory, 'Product.cs'), `using System.Reflection;
[assembly: AssemblyVersion("${config.version}.0")]
[assembly: AssemblyFileVersion("${config.version}.0")]
[assembly: AssemblyInformationalVersion("${config.version}")]
[assembly: AssemblyProduct(${literal(config.productName)})]
namespace ColorfulControlCenter.Installer {
    internal static class Product {
${constants.join('\n')}
    }
}\n`);
  return fields;
}

export function wpfColor(value) {
  if (/^#[\da-f]{6}$/i.test(value)) return value;
  const rgba = value?.match(/^rgba\((\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\)$/);
  if (rgba) return '#' + [Math.round(Number(rgba[4]) * 255), ...rgba.slice(1, 4).map(Number)]
    .map(number => number.toString(16).padStart(2, '0')).join('');
  const hsl = value?.match(/^hsl\(0\s+0%\s+(\d+)%\)$/);
  const cool = value?.match(/^hsl\(240\s+16%\s+(\d+)%\)$/);
  if (hsl || cool) {
    const light = Number((hsl ?? cool)[1]) / 100;
    const chroma = cool ? (1 - Math.abs(2 * light - 1)) * 0.16 : 0;
    const base = light - chroma / 2;
    return '#' + [base, base, base + chroma].map(n => Math.round(n * 255).toString(16).padStart(2, '0')).join('');
  }
  throw new Error(`Unsupported installer token color: ${value}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const arguments_ = process.argv.slice(2);
  if (arguments_.length !== 3 || arguments_.some(value => !value))
    throw new Error('Usage: prepare-installer.mjs <engine> <embedded-application-payload> <build-directory>');
  const [engine, applicationPayload, directory] = arguments_;
  console.log(JSON.stringify(prepareInstaller(engine, applicationPayload, directory)));
}
