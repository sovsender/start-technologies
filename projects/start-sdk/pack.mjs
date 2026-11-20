#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { rmSync } from 'node:fs'
import { preservedCapabilities } from './check-cli.mjs'

const [output, ...args] = process.argv.slice(2)
try {
  execFileSync('start-cli', ['s9pk', 'pack', ...args, '-o', output], {
    stdio: 'inherit',
  })
  if (preservedCapabilities.length) {
    const manifest = JSON.parse(
      execFileSync(
        'start-cli',
        ['s9pk', 'inspect', output, 'cat', 'manifest.json'],
        {
          encoding: 'utf8',
        },
      ),
    )
    for (const key of preservedCapabilities) {
      if (manifest[key] !== true)
        throw new Error(
          `The packer did not preserve ${key}. Upgrade start-cli to a build that supports it.`,
        )
    }
  }
} catch (error) {
  rmSync(output, { force: true })
  throw error
}
