#!/usr/bin/env bash
set -e
elf="$1"                      # cargo passes the built ELF path
rust-objcopy -O ihex "$elf" "$elf.hex"
nrfutil nrf5sdk-tools pkg generate --hw-version 52 --sd-req 0x00 \
  --application "$elf.hex" --application-version 1 "$elf.zip" > /dev/null
nrfutil device program --firmware "$elf.zip" --traits nordicDfu