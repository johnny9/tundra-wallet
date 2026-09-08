#!/usr/bin/env python3
"""Inspect a disposable CI emulator hierarchy, without treating a launcher ANR as app success."""
import re
import sys
import xml.etree.ElementTree as ET


def probe(path):
    nodes = list(ET.parse(path).getroot().iter('node'))
    texts = {node.get('text') for node in nodes if node.get('package') == 'dev.johnny9.tundra.dev'}
    if {'150 BTC', 'Draft · unsigned · 2 inputs'} <= texts:
        return 'ready'
    # A known hosted-emulator failure. Never dismiss a Tundra ANR or arbitrary dialog.
    if any(node.get('package') == 'android' and node.get('text') == "Quickstep isn't responding" for node in nodes):
        for node in nodes:
            if node.get('package') == 'android' and node.get('text') == 'Close app':
                match = re.fullmatch(r'\[(\d+),(\d+)\]\[(\d+),(\d+)\]', node.get('bounds', ''))
                if match:
                    left, top, right, bottom = map(int, match.groups())
                    if 0 <= left < right <= 10000 and 0 <= top < bottom <= 10000:
                        return f'launcher-anr {(left + right) // 2} {(top + bottom) // 2}'
    return 'waiting'


if __name__ == '__main__':
    print(probe(sys.argv[1]))
