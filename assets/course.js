/* Offline enhancement only. Browser attempts are not evidence of mastery. */
(() => {
  'use strict';
  const prefix = 'rusty-course-v1:';
  const read = key => { try { return localStorage.getItem(prefix + key); } catch { return null; } };
  const write = (key, value) => { try { localStorage.setItem(prefix + key, value); } catch { /* Optional storage. */ } };
  const lesson = document.querySelector('[data-lesson]');
  if (lesson) {
    const id = lesson.dataset.lesson;
    const answer = lesson.querySelector('[data-recall]');
    const complete = lesson.querySelector('[data-complete]');
    answer.value = read('answer-' + id) || '';
    complete.checked = read('attempt-' + id) === 'yes';
    answer.addEventListener('input', () => write('answer-' + id, answer.value));
    complete.addEventListener('change', () => write('attempt-' + id, complete.checked ? 'yes' : 'no'));
    lesson.querySelector('[data-check]').addEventListener('click', () => {
      const status = lesson.querySelector('.attempt-message');
      if (!answer.value.trim()) {
        status.textContent = 'Try a prediction first, or write “I don’t know” so your tutor can help.';
        answer.focus();
        return;
      }
      const panel = lesson.querySelector('.feedback');
      panel.hidden = false;
      lesson.querySelector('[data-check]').setAttribute('aria-expanded', 'true');
      status.textContent = 'Feedback revealed. Compare the reasoning, then test one prediction.';
    });
  }
  document.querySelectorAll('[data-exercise]').forEach(exercise => {
    const key = exercise.dataset.exercise;
    const attempt = exercise.querySelector('[data-exercise-attempt]');
    attempt.value = read(key) || '';
    attempt.addEventListener('input', () => write(key, attempt.value));
    exercise.querySelector('[data-exercise-check]').addEventListener('click', () => {
      const status = exercise.querySelector('.exercise-status');
      if (!attempt.value.trim()) {
        status.textContent = 'Write a prediction or an honest “I don’t know” before comparing.';
        attempt.focus(); return;
      }
      exercise.querySelector('.solution').hidden = false;
      exercise.querySelector('[data-exercise-check]').setAttribute('aria-expanded','true');
      status.textContent = 'Solution revealed. Compare the mechanism, then run the verification check.';
    });
  });
  document.querySelectorAll('pre').forEach(pre => {
    if (pre.closest('.expected') || pre.closest('[data-lab]')) return;
    const code = pre.querySelector('code');
    if (!code) return;
    const copy = document.createElement('button');
    copy.type = 'button'; copy.className = 'copy-code'; copy.textContent = 'Copy';
    copy.setAttribute('aria-label','Copy this code or command block');
    copy.addEventListener('click', async () => {
      try {
        if (!navigator.clipboard) throw new Error('Clipboard unavailable');
        await navigator.clipboard.writeText(code.textContent);
        copy.textContent = 'Copied';
      } catch {
        const selection = window.getSelection();
        const range = document.createRange(); range.selectNodeContents(code);
        selection.removeAllRanges(); selection.addRange(range);
        copy.textContent = 'Selected — press Copy';
      }
    });
    pre.before(copy);
  });
  const filter = document.querySelector('[data-filter]');
  if (filter) {
    const items = Array.from(document.querySelectorAll('[data-course-item]'));
    const update = () => {
      const query = filter.value.trim().toLowerCase();
      let count = 0;
      let attempts = 0;
      items.forEach(item => {
        item.hidden = !item.dataset.search.includes(query);
        if (!item.hidden) count += 1;
        const done = read('attempt-' + item.dataset.courseItem) === 'yes';
        if (done) attempts += 1;
        const badge = item.querySelector('.attempt-badge');
        badge.textContent = done ? 'Tried' : '';
        badge.setAttribute('aria-label', done ? 'Marked attempted, not graded' : 'Not yet marked attempted');
      });
      document.querySelectorAll('.stage').forEach(stage => {
        stage.hidden = !Array.from(stage.querySelectorAll('[data-course-item]')).some(item => !item.hidden);
      });
      document.querySelector('[data-progress]').textContent = `${count} of ${items.length} lessons shown · ${attempts} marked attempted`;
      document.querySelector('.empty-search').hidden = count !== 0;
    };
    filter.addEventListener('input', update);
    window.addEventListener('pageshow', update);
    document.querySelector('[data-reset]').addEventListener('click', () => {
      if (!window.confirm('Clear this course’s saved answers and attempt marks in this browser?')) return;
      try {
        const keys = [];
        for (let i = 0; i < localStorage.length; i += 1) {
          const key = localStorage.key(i);
          if (key && key.startsWith(prefix)) keys.push(key);
        }
        keys.forEach(key => localStorage.removeItem(key));
      } catch { /* Optional storage. */ }
      update();
    });
    update();
  }
})();
