#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageName = 'dent-cli';
const registryUrl = process.env.DENT_NPM_REGISTRY_URL || `https://registry.npmjs.org/${packageName}/latest`;
const skillDir = dirname(dirname(fileURLToPath(import.meta.url)));
const skillText = readFileSync(join(skillDir, 'SKILL.md'), 'utf8');
const installedVersion = skillText.match(/^version:\s*['"]?([^'"\n]+)['"]?$/m)?.[1]?.trim() || 'unknown';

function compareVersions(left, right) {
  const a = String(left || '0').split(/[.-]/).map(part => Number.parseInt(part, 10) || 0);
  const b = String(right || '0').split(/[.-]/).map(part => Number.parseInt(part, 10) || 0);
  const length = Math.max(a.length, b.length);
  for (let index = 0; index < length; index += 1) {
    if ((a[index] || 0) > (b[index] || 0)) return 1;
    if ((a[index] || 0) < (b[index] || 0)) return -1;
  }
  return 0;
}

try {
  const response = await fetch(registryUrl, { headers: { Accept: 'application/json' } });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const payload = await response.json();
  const latestVersion = payload.version;
  if (!latestVersion) throw new Error('registry response did not include version');
  if (installedVersion === 'unknown' || compareVersions(installedVersion, latestVersion) < 0) {
    console.log(`Dent skill update available: installed v${installedVersion}, npm v${latestVersion}.`);
    console.log('Run `npx dent-cli update` or `dent update`.');
  } else {
    console.log(`Dent skill is current: v${installedVersion}.`);
  }
} catch (error) {
  console.error(`Could not check npm registry for dent-cli: ${error.message}`);
  process.exit(1);
}
