#!/usr/bin/env node

import { spawnSync } from 'node:child_process';

function run(args) {
  const result = spawnSync('dent', args, { encoding: 'utf8' });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  return result.status ?? 1;
}

const checkStatus = run(['check']);
const authStatus = run(['whoami']);
process.exit(checkStatus === 0 && authStatus === 0 ? 0 : 1);
