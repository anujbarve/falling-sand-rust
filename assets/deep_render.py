"""Render authored long-form lessons using the existing offline course shell."""
from pathlib import Path
import html
import json
import re

ROOT = Path(__file__).resolve().parent.parent
E = html.escape


def load_deep_lessons():
    result = {}
    for path in sorted((ROOT/'assets').glob('deep-[0-9]*.json')):
        payload = json.loads(path.read_text())
        for key, value in payload.items():
            if key in result:
                raise ValueError(f'Duplicate detailed lesson {key} in {path}')
            result[key] = value
    return result


def diagram_for(row):
    path=ROOT/'assets/diagrams/manifest.json'
    if not path.exists(): return ''
    diagrams=json.loads(path.read_text())
    n=row['number']
    direct=[d for d in diagrams.values() if n in d.get('lesson_ids',[])]
    if not direct:
        return ""
    if n in (56,57,58,59,60):
        direct=[diagrams["random-ticks-calendar"]]
    d=direct[0]
    assert d.get('verified'), f'Unverified diagram for lesson {n}'
    return f'''<figure class="concept-diagram"><img src="../assets/diagrams/{E(d['file'])}" alt="{E(d['caption'])}" loading="lazy" width="1200" height="620"><figcaption><strong>{E(d['title'])}.</strong> {E(d['caption'])}</figcaption></figure>'''


def lab_for(n):
    kind=None
    if n in (1,2): kind='toolchain'
    elif n in (10,17,18,19,20,22,23): kind='grid'
    elif n in (12,13,40): kind='borrow'
    elif n in (14,15,56,57,58,59,60): kind='ticks'
    elif n in (76,77,78): kind='snapshot'
    if not kind:return ''
    return f'''<section class="lab-interactive" id="interactive-lab"><p class="eyebrow">Try the idea here</p><div data-lab="{kind}"></div><noscript><p>This optional interactive model needs JavaScript. The diagram and worked example above teach the same mechanism without it.</p></noscript><p class="small">This is a bounded teaching model in your browser, not your Rust game and not a real terminal. It never executes shell commands or modifies project files.</p></section>'''


def exercise_html(n,i,exercise):
    uid=f'exercise-{n}-{i}'
    return f'''<article class="exercise" data-exercise="{uid}"><div class="exercise-heading"><span>{i:02d}</span><h3>{E(exercise['title'])}</h3></div>
<p>{E(exercise['prompt'])}</p>
<details class="hint"><summary>Need a hint?</summary><p>{E(exercise['hint'])}</p></details>
<label for="{uid}-attempt">Your attempt, prediction or observed output</label><textarea id="{uid}-attempt" data-exercise-attempt rows="3" placeholder="Try first. “I don’t know” is a useful answer too."></textarea>
<button type="button" data-exercise-check aria-expanded="false" aria-controls="{uid}-solution">Reveal worked solution</button><p class="exercise-status" aria-live="polite"></p>
<div class="solution" id="{uid}-solution" hidden><h4>Worked solution</h4>{exercise['solution']}<h4>How to verify your result</h4><p>{E(exercise['proof'])}</p></div></article>'''


def render_deep_lesson(row,previous,following,deep,*,shell,name,stages,lab_titles):
    n,title,stage=row['number'],row['title'],row['stage']
    sid=lambda text:re.sub(r'[^a-z0-9]+','-',text.lower()).strip('-')
    nav_prev='<a href="index.html">← Course map</a>' if previous is None else f'<a rel="prev" href="{name(previous)}">← {previous["number"]:02d} · {E(previous["title"])}</a>'
    nav_next='<a href="index.html">Course map →</a>' if following is None else f'<a rel="next" href="{name(following)}">{following["number"]:02d} · {E(following["title"])} →</a>'
    prior='No programming experience is assumed. Start with the macOS path if you are using your MacBook.' if previous is None else f'Previously: <a href="{name(previous)}">{E(previous["title"])}</a>. Keep that working result available before starting this lesson.'
    toc=[('purpose','What you will make')]
    sections=''
    for i,s in enumerate(deep['sections'],1):
        section_id=f'concept-{i}-{sid(s["title"])}'
        toc.append((section_id,s['title']))
        sections+=f'<section class="explanation" id="{section_id}"><h2><span class="section-index">{i:02d}</span>{E(s["title"])}</h2>{s["html"]}</section>'
    toc.extend([('worked-example','Worked example'),('practice','Exercises & solutions'),('troubleshooting','When it goes wrong'),('proof','Finish and verify')])
    contents=''.join(f'<li><a href="#{anchor}">{E(label)}</a></li>' for anchor,label in toc)
    worked=deep['worked']
    extension={'rust':'rs','sh':'sh','bash':'sh','yaml':'yml','toml':'toml','python':'py'}.get(worked.get('language','text').lower(),'txt')
    download_name=f'lesson_{n:03d}.{extension}'
    (ROOT/'assets/worked').mkdir(exist_ok=True)
    (ROOT/'assets/worked'/download_name).write_text(worked['code']+'\n')
    lines=''.join(f'<li>{E(x)}</li>' for x in worked['explanation'])
    examples=f'''<section id="worked-example"><p class="eyebrow">Read → predict → run → change</p><h2>Walk through the working example</h2><p>{E(worked['label'])}</p><p class="download-example"><a href="../assets/worked/{download_name}" download>Download this worked example ↧</a></p><pre data-language="{E(worked.get('language','text'))}"><code>{E(worked['code'])}</code></pre><h3>Read what each part does</h3><ol class="walkthrough">{lines}</ol><div class="expected"><h3>Expected result</h3><pre><code>{E(worked['expected'])}</code></pre><p>Exact version numbers, paths and compiler wording may differ. Compare the behavior and file locations, not incidental output formatting.</p></div></section>'''
    exercises=''.join(exercise_html(n,i,ex) for i,ex in enumerate(deep['exercises'],1))
    troubles=''.join(f'<details class="trouble"><summary>{E(t["symptom"])}</summary><p><strong>What is happening:</strong> {E(t["cause"])}</p><p><strong>Try this:</strong> {E(t["fix"])}</p></details>' for t in deep['troubleshooting'])
    links=''.join(f'<li><a href="{E(s["url"])}">{E(s["label"])}</a></li>' for s in deep['sources'])
    notices=''
    if title in lab_titles:
        notices+='<aside class="notice"><strong>Design choice to settle.</strong> The current specification leaves a decision here. The example makes the mechanism concrete; document the chosen behavior before treating it as the final game rule.</aside>'
    if previous and previous['stage']!=stage:
        notices+=f'<aside class="notice"><strong>{stage} starts here.</strong> Review this milestone’s implementation plan against the current spec. The lessons teach the work; they do not silently approve new game behavior.</aside>'
    words=len(re.findall(r'\b\w+\b',re.sub('<[^>]+>',' ',json.dumps(deep,ensure_ascii=False))))
    reading=max(5,round(words/190))
    body=f'''<article class="lesson lesson-deep" data-lesson="{n}"><p class="eyebrow">{E(stage)} / {E(stages[stage][0])}<span>Lesson {n:02d} of 100 · ~{reading} min reading + build time</span></p><h1>{E(title)}</h1><p class="lede">{E(deep['outcome'])}</p><p class="prerequisite">{prior}</p>{notices}
<div class="deep-layout"><div class="deep-main"><section id="purpose" class="lesson-purpose"><h2>Why we need this now</h2><p>{E(deep['why'])}</p><p><strong>Your result:</strong> {E(deep['outcome'])}</p></section>
{diagram_for(row)}{sections}{examples}{lab_for(n)}
<section id="practice"><p class="eyebrow">Prediction · repair · transfer</p><h2>Exercises you actually do</h2><p>Write a prediction or make an attempt before opening a solution. Use the hint if stuck; then compare reasoning and verify with the stated check. A different implementation is fine when it satisfies the same contract.</p>{exercises}</section>
<section id="troubleshooting"><h2>When it goes wrong</h2><p>Start with the observed symptom. Change one assumption at a time instead of pasting fixes until the error disappears.</p>{troubles}</section>
<section class="recall" id="recall"><h2>Explain it without the example</h2><p id="question-{n}">{E(row['question'])}</p><label for="answer-{n}">Your reasoning</label><textarea id="answer-{n}" rows="3" data-recall placeholder="Use your own words or trace a concrete example."></textarea><button type="button" data-check aria-expanded="false" aria-controls="feedback-{n}">Compare your explanation</button><p class="attempt-message" aria-live="polite"></p><div class="feedback" id="feedback-{n}" hidden><h3>Reasoning to compare</h3><p>{E(row['feedback'])}</p><p>Self-check only: test a prediction or ask your tutor if anything still feels arbitrary.</p></div></section>
<section id="proof"><h2>Finish with evidence</h2><p>{E(row['proof'])}</p><p>Make one small variation without copying: predict it, run it and explain any difference. On another day, repeat one exercise from memory before starting the next lesson. Ask your tutor about any step you can execute but cannot explain.</p><label class="completion"><input type="checkbox" data-complete> I attempted the exercises and kept evidence of the result.</label><p class="small">This is an attempt marker, not a grade. Learning records are written only after demonstrated understanding.</p></section>
<section id="sources"><h2>Primary sources for this lesson</h2><ul class="sources-list">{links}</ul><p>Use sources to verify the mechanism. The lesson itself supplies the instructions; the links are not a substitute for teaching them.</p></section></div>
<aside class="lesson-contents"><p class="eyebrow">On this page</p><ol>{contents}</ol><a class="reference-shortcut" href="../reference/index.html">Reference shelf ↗</a><p>Ask here whenever something is unclear. Show the command, code, expected result and actual result.</p><a href="#main">Back to top ↑</a></aside></div><nav class="lesson-nav" aria-label="Lesson navigation">{nav_prev}{nav_next}</nav></article>'''
    return shell(title,body,'detailed-course')
