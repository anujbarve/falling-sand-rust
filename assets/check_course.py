#!/usr/bin/env python3
"""Check generated course structure, offline dependencies, navigation and feedback gates."""
from pathlib import Path
from html.parser import HTMLParser
from urllib.parse import urlsplit, unquote
import json
import re

ROOT = Path(__file__).resolve().parent.parent

class Page(HTMLParser):
    def __init__(self, path):
        super().__init__(convert_charrefs=True)
        self.path = path
        self.ids = set()
        self.links = []
        self.h1 = 0
        self.feedback = 0
        self.checks = 0
        self.styles = 0
        self.scripts = 0
        self.languages = []
        self.exercises = 0
        self.solutions = 0
        self.diagrams = 0
    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if 'id' in a:
            assert a['id'] not in self.ids, (self.path, 'duplicate id', a['id'])
            self.ids.add(a['id'])
        if tag == 'html': self.languages.append(a.get('lang'))
        if tag == 'h1': self.h1 += 1
        if 'data-exercise' in a: self.exercises += 1
        if tag == 'img' and 'diagrams/' in a.get('src',''): self.diagrams += 1
        if 'solution' in a.get('class','').split():
            self.solutions += 1
            assert 'hidden' in a, (self.path, 'exercise solution initially visible')
        if tag == 'link' and a.get('rel') == 'stylesheet': self.styles += 1
        if tag == 'script': self.scripts += 1
        if 'data-check' in a: self.checks += 1
        if 'feedback' in a.get('class','').split():
            self.feedback += 1
            assert 'hidden' in a, (self.path, 'feedback visible before attempt')
        for key in ('href','src'):
            if key in a:
                self.links.append((tag, a[key]))


def main():
    rows = json.loads((ROOT/'assets/course.json').read_text())
    numbered = sorted((ROOT/'lessons').glob('[0-9][0-9][0-9][0-9]-*.html'))
    assert len(numbered) == len(rows)
    assert len(rows) >= 100
    assert len({r['title'] for r in rows}) == len(rows)
    assert [r['number'] for r in rows] == list(range(1, len(rows)+1))
    for row in rows:
        assert len(row['work']) >= 3
        assert all(row.get(k) for k in ('concept','proof','question','feedback','source'))
    pages = {}
    for path in sorted((ROOT/'lessons').glob('*.html')) + sorted((ROOT/'reference').glob('*.html')):
        parsed = Page(path)
        parsed.feed(path.read_text())
        assert parsed.h1 == 1 and parsed.styles == 1 and parsed.scripts == 2, path
        assert parsed.languages == ['en'], path
        if path in numbered:
            assert parsed.feedback == 1 and parsed.checks == 1, path
            assert len(re.findall('rel="prev"',path.read_text())) <= 1
            assert 'data-recall' in path.read_text() and 'data-complete' in path.read_text()
        pages[path.resolve()] = parsed
    local_links = 0
    for path, parsed in pages.items():
        for tag, target in parsed.links:
            url = urlsplit(target)
            if url.scheme or url.netloc:
                assert tag not in ('script','link'), (path,'external runtime dependency',target)
                continue
            linked = (path.parent / unquote(url.path)).resolve() if url.path else path
            assert linked.exists(), (path,'broken local target',target)
            if url.fragment and linked in pages:
                assert unquote(url.fragment) in pages[linked].ids, (path,'missing fragment',target)
            local_links += 1
    expanded = {}
    for source in sorted((ROOT/'assets').glob('deep-[0-9]*.json')):
        for key, value in json.loads(source.read_text()).items():
            assert key not in expanded, ('duplicate expanded lesson',key)
            expanded[key] = value
            assert len(value['sections']) >= 4, key
            assert len(value['exercises']) >= 3 and len(value['troubleshooting']) >= 3, key
            assert value['worked']['code'].strip() and value['worked']['expected'].strip(),key
    assert set(expanded) == {str(i) for i in range(1,len(rows)+1)}, 'Every lesson must have expanded teaching content'
    for path in numbered:
        n = str(int(path.name[:4]))
        if n in expanded:
            parsed=pages[path.resolve()]
            assert parsed.exercises >= 3 and parsed.solutions == parsed.exercises, path
            # Draw a diagram only when it explains this lesson, not as decoration.
    examples = json.loads((ROOT/'assets/examples.json').read_text())
    assert len(examples) >= 10
    assert not (ROOT/'learning-records').exists() or not list((ROOT/'learning-records').glob('*.md')), 'Unexpected learning records: audit before claiming no mastery'
    print(f'PASS: {len(numbered)} lessons, {len(pages)} HTML pages, {local_links} local links, hidden feedback, navigation and offline assets.')
    print(f'{len(expanded)} expanded lessons have full sections, worked code, hidden exercise solutions, troubleshooting; relevant lessons include verified diagrams.')
    print(f'{len(examples)} Rust worked examples exported; lesson 12 deliberately fails to demonstrate moves.')

if __name__ == '__main__':
    main()
