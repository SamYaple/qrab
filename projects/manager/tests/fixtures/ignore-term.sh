#!/bin/sh
# SIG_IGN survives exec, so the supervisor must exercise its SIGKILL path.
trap '' TERM
exec /bin/sleep 1000
