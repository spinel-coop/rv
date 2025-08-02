#!/bin/sh -eu
NUMSECTORS=$(($1*1024*1024/512))
MYDEV=$(hdiutil attach -nomount ram://$NUMSECTORS)
diskutil eraseVolume HFS+ "ramdisk-${1}mb" $MYDEV