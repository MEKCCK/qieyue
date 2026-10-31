'use strict';

/* ---------------------------------------------------------------------------
 * QieYue — Tauri v2 frontend.
 *
 * All protocol/crypto work stays in Rust (`fqapi_core` + the `qieyue` adapter);
 * this file only renders state and posts commands. Navigation mirrors the
 * desktop shell: four root tabs, each with its own page stack.
 * ------------------------------------------------------------------------- */

const tauri = window.__TAURI__;
if (!tauri || !tauri.core || !tauri.core.invoke) {
  document.body.innerHTML =
    '<div class="empty">Tauri 未注入：请通过 QieYue.app 或 cargo run -p qieyue 启动。</div>';
  throw new Error('Tauri API missing');
}
const invoke = tauri.core.invoke;
const convertFileSrc = tauri.core.convertFileSrc || ((path) => `file://${path}`);

const TABS = [
  { id: 'home', label: '首页', icon: 'home' },
  { id: 'library', label: '书架', icon: 'library' },
  { id: 'rank', label: '排行榜', icon: 'rank' },
  { id: 'category', label: '分类', icon: 'category' },
];

const ICONS = {
  home: '<svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M2 8l7-6 7 6"/><path d="M4 7v8h10V7"/><path d="M7.5 15v-4h3v4"/></svg>',
  library: '<svg viewBox="0 0 18 18" fill="currentColor"><rect x="2" y="4" width="3" height="11" rx="1"/><rect x="7" y="6" width="3" height="9" rx="1"/><rect x="12" y="2" width="3" height="13" rx="1"/></svg>',
  rank: '<svg viewBox="0 0 18 18" fill="currentColor"><rect x="2" y="10" width="3" height="6" rx="1"/><rect x="7" y="6" width="3" height="10" rx="1"/><rect x="12" y="2" width="3" height="14" rx="1"/></svg>',
  category: '<svg viewBox="0 0 18 18" fill="currentColor"><rect x="2" y="2" width="5.5" height="5.5" rx="1.2"/><rect x="10.5" y="2" width="5.5" height="5.5" rx="1.2"/><rect x="2" y="10.5" width="5.5" height="5.5" rx="1.2"/><rect x="10.5" y="10.5" width="5.5" height="5.5" rx="1.2"/></svg>',
};

const TITLES = {
  home: '首页', library: '书架', rank: '排行榜', results: '搜索结果',
  book: '书籍详情', series: '短剧', reader: '阅读', manga: '漫画', category: '分类',
};

const ROOT_VIEW = { home: 'home', library: 'library', rank: 'rank', category: 'category' };

const state = {
  tab: 'home',
  view: 'home',
  stacks: {
    home: [{ view: 'home' }],
    library: [{ view: 'library' }],
    rank: [{ view: 'rank' }],
    category: [{ view: 'category' }],
  },
  query: '',
  searchQuery: '',
  home: null,
  homeTabIndex: 0,
  hits: [],
  searchNext: null,
  shelf: [],
  book: null,
  chapters: [],
  chapterIndex: 0,
  chapterTitle: '',
  paragraphs: [],
  series: null,
  rank: null,
  rankId: '',
  rankAlgo: '101',
  rankCategory: '0',
  rankHits: [],
  rankMore: true,
  category: null,
  categoryTab: 1,
  comments: [],
  commentsNext: null,
  mangaPages: [],
  readerScroll: 0,
  pendingItem: null,
  status: '',
  homeLoadingMore: false,
  searchLoadingMore: false,
  rankLoadingMore: false,
  userScrolled: false,
  lastAutoPage: 0,
};

/* ---- requests ------------------------------------------------------------ */

const latest = Object.create(null);
const pending = Object.create(null);
let seq = 0;
let scrollScheduled = false;
let toastTimer = 0;

const MAX_INFLIGHT = 8;
let inflight = 0;
const slotWaiters = [];

async function acquireSlot() {
  if (inflight < MAX_INFLIGHT) {
    inflight += 1;
    return;
  }
  await new Promise((resolve) => slotWaiters.push(resolve));
  inflight += 1;
}

function releaseSlot() {
  inflight = Math.max(0, inflight - 1);
  const next = slotWaiters.shift();
  if (next) next();
}

function busy() {
  return Object.values(pending).some((n) => n > 0);
}

function updateBusy() {
  const isBusy = busy();
  $('#spinner').classList.toggle('hidden', !isBusy);
  $('#status-text').textContent = state.status;
  $('.status').classList.toggle('busy', isBusy);
}

function setStatus(text) {
  state.status = text;
  updateBusy();
}

function errorText(err) {
  if (typeof err === 'string') return err;
  if (err && typeof err.message === 'string') return err.message;
  try { return JSON.stringify(err); } catch { return String(err); }
}

async function call(domain, fn, handlers = {}) {
  const id = ++seq;
  latest[domain] = id;
  pending[domain] = (pending[domain] || 0) + 1;
  updateBusy();
  await acquireSlot();
  try {
    const result = await fn();
    if (latest[domain] === id && handlers.onOk) handlers.onOk(result);
  } catch (err) {
    if (latest[domain] === id) {
      const message = errorText(err);
      if (handlers.onErr) handlers.onErr(message);
      else toast(message);
    }
  } finally {
    releaseSlot();
    pending[domain] = Math.max(0, (pending[domain] || 0) - 1);
    updateBusy();
    if (handlers.onFinally) handlers.onFinally();
  }
}

function busyDomain(domain) {
  return (pending[domain] || 0) > 0;
}

/* ---- helpers ------------------------------------------------------------- */

const $ = (selector, root = document) => root.querySelector(selector);

function esc(value) {
  return String(value ?? '').replace(/[&<>"']/g, (ch) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  }[ch]));
}

function imageUrl(url) {
  if (!url) return '';
  return url.startsWith('//') ? `https:${url}` : url;
}

function coverHtml(url, cls, alt) {
  const src = imageUrl(url);
  if (!src) return `<div class="${cls} placeholder"></div>`;
  return `<img class="${cls}" src="${esc(src)}" alt="${esc(alt)}" loading="lazy" draggable="false">`;
}

function localSrc(uri) {
  const path = String(uri).replace(/^file:\/\//, '');
  try { return convertFileSrc(path); } catch { return uri; }
}

function loadingScreen(text) {
  return `<div class="loading-screen"><span class="spinner"></span><span>${esc(text)}</span></div>`;
}

function logDebug(message) {
  try { invoke('debug_log', { message: String(message) }).catch(() => {}); } catch { /* ignore */ }
}

function toast(message) {
  logDebug(`toast: ${message}`);
  const el = $('#toast');
  el.textContent = message;
  el.classList.remove('hidden');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.add('hidden'), 3600);
}

/* ---- navigation ---------------------------------------------------------- */

function stack() { return state.stacks[state.tab]; }
function rootOf(tab) { return ROOT_VIEW[tab]; }

function ownerFor(view) {
  if (view === 'book' || view === 'reader' || view === 'manga') return state.book?.id ?? null;
  if (view === 'series') return state.series?.id ?? null;
  if (view === 'results') return state.searchQuery;
  return null;
}

function pageMatches(page) {
  return page.owner == null || page.owner === ownerFor(page.view);
}

function canBack() { return stack().length > 1; }

function go(view) {
  if (state.view !== view) {
    stack().push({ view, owner: ownerFor(view) });
    state.view = view;
    render();
    loadView(view);
  }
}

function back() {
  const s = stack();
  if (s.length > 1) s.pop();
  while (s.length > 1 && !pageMatches(s[s.length - 1])) s.pop();
  state.view = s[s.length - 1].view;
  render();
  loadView(state.view);
}

function selectTab(tab) {
  if (tab === state.tab) {
    stack().length = 1;
    state.view = rootOf(tab);
    render();
    loadTab(tab);
    return;
  }
  state.tab = tab;
  const s = stack();
  while (s.length > 1 && !pageMatches(s[s.length - 1])) s.pop();
  state.view = s[s.length - 1].view;
  render();
  loadTab(tab);
}

function openDirectory() {
  if (!state.book) return;
  const s = stack();
  const index = s.map((p) => p.view).lastIndexOf('book');
  if (index >= 0) {
    s.length = index + 1;
    state.view = 'book';
    render();
    loadView('book');
  } else {
    go('book');
  }
}

/* ---- loaders ------------------------------------------------------------- */

function loadTab(tab) {
  if (tab === 'home') {
    if (!state.home) loadHome();
  } else if (tab === 'library') {
    refreshShelf();
  } else if (tab === 'rank') {
    if (!state.rank) loadRankCatalog();
    else if (!state.rankHits.length) loadRankPage(0);
  } else if (tab === 'category') {
    if (!state.category) loadCategory(state.categoryTab);
  }
}

function loadView(view) {
  if (view === 'home' || view === 'library' || view === 'rank' || view === 'category') {
    loadTab(state.tab);
  }
}

async function refreshShelf() {
  try {
    state.shelf = await invoke('load_shelf');
  } catch (err) {
    state.shelf = [];
    toast(errorText(err));
  }
  render({ keepScroll: true });
}

function loadHome() {
  setStatus('加载首页…');
  call('home', () => invoke('home'), {
    onOk: (home) => {
      state.home = home;
      state.homeTabIndex = home.active;
      for (const section of home.sections) section._visible = 60;
      setStatus(`${home.sections.length} 个推荐区块 · ${home.sections.reduce((n, s) => n + s.hits.length, 0)} 张卡片`);
      logDebug(`home loaded: ${home.sections.length} sections`);
      render();
    },
    onErr: toast,
  });
}

function loadHomeChannel(tabType, index, template) {
  setStatus('加载频道…');
  call('home', () => invoke('home_channel', {
    tab_type: tabType, tab_index: index, client_template: template,
  }), {
    onOk: (home) => {
      state.home = home;
      state.homeTabIndex = index;
      for (const section of home.sections) section._visible = 60;
      setStatus(`${home.sections.length} 个推荐区块`);
      render();
    },
    onErr: toast,
  });
}

function loadHomeMore(sectionIndex) {
  const section = state.home?.sections?.[sectionIndex];
  if (!section || section.next_offset == null || state.homeLoadingMore) return;
  const homeGen = latest.home;
  state.homeLoadingMore = true;
  setStatus('加载更多…');
  call('homeMore', () => invoke('home_more', {
    cell_id: section.cell_id,
    tab_type: section.tab_type,
    client_template: section.client_template,
    offset: section.next_offset,
  }), {
    onOk: ([hits, next]) => {
      if (latest.home !== homeGen) return;
      const target = state.home?.sections?.[sectionIndex];
      if (!target) return;
      const seen = new Set(target.hits.map((hit) => hit.id));
      const fresh = [];
      for (const hit of hits) {
        if (!seen.has(hit.id)) { seen.add(hit.id); fresh.push(hit); }
      }
      target.hits.push(...fresh);
      // A page with no new rows means the block is exhausted, even if the
      // service still advertises an offset. Otherwise it would page forever.
      target.next_offset = fresh.length === 0 || target.hits.length >= 240 ? null : next;
      if (fresh.length) {
        target._visible = Math.min((target._visible || 60) + fresh.length, target.hits.length);
        const grid = document.getElementById(`home-grid-${sectionIndex}`);
        if (grid) grid.insertAdjacentHTML('beforeend', fresh.map(homeCardHtml).join(''));
      }
      updateSectionMoreButton(target, sectionIndex);
      updateHomeMoreWrap();
      setStatus(`已加载 ${target.hits.length} 张`);
    },
    onErr: (message) => {
      const target = state.home?.sections?.[sectionIndex];
      if (target) target.next_offset = null;
      updateSectionMoreButton(target, sectionIndex);
      updateHomeMoreWrap();
      toast(message);
    },
    onFinally: () => { state.homeLoadingMore = false; },
  });
}

function submitSearch(query) {
  const value = String(query || '').trim();
  if (!value) return;
  state.query = value;
  state.searchQuery = value;
  $('#search-input').value = value;
  setStatus('搜索中…');
  call('search', () => invoke('search_page', { query: value, offset: 0, tab_type: null }), {
    onOk: (page) => {
      state.hits = page.hits;
      state.searchNext = page.next_offset;
      setStatus(`${page.hits.length} 条结果`);
      logDebug(`search loaded: ${page.hits.length} hits`);
      go('results');
      render();
    },
    onErr: toast,
  });
}

function loadSearchMore() {
  if (state.searchNext == null || state.searchLoadingMore) return;
  state.searchLoadingMore = true;
  setStatus('加载更多…');
  call('searchMore', () => invoke('search_page', {
    query: state.searchQuery, offset: state.searchNext, tab_type: null,
  }), {
    onOk: (page) => {
      const seen = new Set(state.hits.map((hit) => hit.id));
      const fresh = [];
      for (const hit of page.hits) {
        if (!seen.has(hit.id)) { seen.add(hit.id); fresh.push(hit); }
      }
      state.hits.push(...fresh);
      state.searchNext = fresh.length ? page.next_offset : null;
      if (fresh.length) {
        const list = document.getElementById('results-list');
        if (list) list.insertAdjacentHTML('beforeend', fresh.map(resultRowHtml).join(''));
      }
      updateResultsMore();
      setStatus(`${state.hits.length} 条结果`);
    },
    onErr: toast,
    onFinally: () => { state.searchLoadingMore = false; },
  });
}

function loadRankCatalog() {
  setStatus('加载榜单…');
  call('rank', () => invoke('rank_catalog'), {
    onOk: (catalog) => {
      state.rank = catalog;
      state.rankId = catalog.rank_id;
      state.rankAlgo = catalog.boards[0]?.algo || '101';
      state.rankCategory = '0';
      setStatus(`${catalog.boards.length} 个榜单 · ${catalog.categories.length} 个分类`);
      loadRankPage(0);
      render();
    },
    onErr: toast,
  });
}

function loadRankPage(offset) {
  if (!state.rankId) return;
  setStatus(offset ? '加载更多…' : '加载榜单…');
  call('rankPage', () => invoke('rank_page', {
    rank_id: state.rankId,
    algo: state.rankAlgo,
    category_id: state.rankCategory,
    offset,
  }), {
    onOk: (hits) => {
      if (!offset) {
        state.rankHits = hits;
        state.rankMore = hits.length > 0;
        setStatus(state.rankMore ? `榜单 ${state.rankHits.length} 条` : '榜单没有数据');
        render();
        return;
      }
      const seen = new Set(state.rankHits.map((hit) => hit.id));
      const fresh = [];
      for (const hit of hits) {
        if (!seen.has(hit.id)) { seen.add(hit.id); fresh.push(hit); }
      }
      const start = state.rankHits.length;
      state.rankHits.push(...fresh);
      state.rankMore = fresh.length > 0;
      if (fresh.length) {
        const list = document.getElementById('rank-list');
        if (list) list.insertAdjacentHTML('beforeend', fresh.map((hit, i) => rankRowHtml(hit, start + i)).join(''));
      }
      updateRankFooter();
      setStatus(state.rankMore ? `榜单 ${state.rankHits.length} 条` : `榜单到底了（${state.rankHits.length} 条）`);
    },
    onErr: toast,
    onFinally: () => { state.rankLoadingMore = false; },
  });
}

function loadRankMore() {
  if (!state.rankMore || state.rankLoadingMore) return;
  state.rankLoadingMore = true;
  loadRankPage(state.rankHits.length);
}

function loadCategory(tab) {
  state.categoryTab = tab;
  setStatus('加载分类…');
  call('category', () => invoke('category_front', { tab }), {
    onOk: (front) => {
      state.category = front;
      setStatus(`${front.tiles.length} 个标签 · ${front.tabs.length} 个频道`);
      render();
    },
    onErr: toast,
  });
}

function openBook(id, pendingItem = null) {
  state.pendingItem = pendingItem;
  setStatus('加载目录…');
  call('book', () => invoke('open_book', { id }), {
    onOk: (detail) => {
      state.book = { ...detail, id };
      state.chapters = detail.chapters;
      state.chapterIndex = 0;
      state.chapterTitle = '';
      state.paragraphs = [];
      state.comments = [];
      state.commentsNext = null;
      logDebug(`book loaded: ${detail.title} / ${detail.chapters.length} chapters`);
      go('book');
      render();
      if (!detail.is_comic) loadComments(0);
      if (state.pendingItem) {
        const target = state.pendingItem;
        state.pendingItem = null;
        const index = state.chapters.findIndex((chapter) => chapter.item_id === target);
        if (index >= 0) openChapter(index);
      }
    },
    onErr: toast,
  });
}

function openSeries(id, cover = '') {
  setStatus('加载剧集…');
  call('series', () => invoke('open_series', { id }), {
    onOk: (series) => {
      if (!series.cover) series.cover = cover;
      state.series = series;
      go('series');
      render();
    },
    onErr: toast,
  });
}

function openChapter(index) {
  const chapter = state.chapters[index];
  if (!chapter) return;
  state.chapterIndex = index;
  if (state.book?.is_comic) {
    openManga(chapter.item_id);
    return;
  }
  setStatus('加载章节…');
  call('chapter', () => invoke('read_chapter', { item_id: chapter.item_id }), {
    onOk: ([title, paragraphs]) => {
      state.chapterTitle = title;
      state.paragraphs = paragraphs;
      state.readerScroll = 0;
      go('reader');
      render();
      if (state.book && inShelf(state.book.id)) {
        invoke('update_progress', {
          book_id: state.book.id,
          item_id: chapter.item_id,
          chapter_title: title,
          index,
        }).then(refreshShelfQuietly).catch(() => {});
      }
    },
    onErr: toast,
  });
}

function openManga(itemId) {
  setStatus('解密漫画…');
  call('manga', () => invoke('manga_chapter', { item_id: itemId }), {
    onOk: (pages) => {
      state.mangaPages = pages.map(localSrc);
      go('manga');
      render();
    },
    onErr: toast,
  });
}

function loadComments(offset) {
  const bookId = state.book?.id;
  if (!bookId) return;
  setStatus(offset ? '加载更多评论…' : '加载评论…');
  call('comments', () => invoke('book_comments', { book_id: bookId, offset }), {
    onOk: ([rows, next]) => {
      if (state.book?.id !== bookId) return;
      if (!offset) {
        state.comments = rows;
        state.commentsNext = next;
        if (state.view === 'book') render({ keepScroll: true });
      } else {
        state.comments.push(...rows);
        state.commentsNext = next;
        if (state.view === 'book') {
          const list = document.getElementById('comments-list');
          if (list) list.insertAdjacentHTML('beforeend', rows.map(commentHtml).join(''));
          updateCommentsMore();
        }
      }
    },
    onErr: toast,
  });
}

function playEpisode(vid, title, episodeIndex) {
  setStatus('解析视频…');
  call('play', () => invoke('play_episode', { vid, title }), {
    onOk: () => {
      setStatus(`mpv 已启动: ${title}`);
      if (state.series && inShelf(state.series.id)) {
        invoke('update_progress', {
          book_id: state.series.id,
          item_id: vid,
          chapter_title: `第 ${episodeIndex} 集`,
          index: Math.max(0, episodeIndex - 1),
        }).then(refreshShelfQuietly).catch(() => {});
      }
    },
    onErr: toast,
  });
}

function playAudio() {
  const book = state.book;
  const chapter = state.chapters[state.chapterIndex];
  if (!book || !chapter) { toast('先打开一本书'); return; }
  setStatus('解析音频…');
  call('play', () => invoke('play_audio', {
    book_id: book.id, item_id: chapter.item_id, title: chapter.title,
  }), {
    onOk: (tone) => setStatus(`mpv 正在朗读（音色 ${tone}）`),
    onErr: toast,
  });
}

async function refreshShelfQuietly() {
  try { state.shelf = await invoke('load_shelf'); } catch { /* ignore */ }
  if (state.view === 'book' || state.view === 'reader' || state.view === 'series') {
    render({ keepScroll: true });
  }
}

function inShelf(id) {
  return state.shelf.some((entry) => entry.id === id);
}

async function toggleShelfCurrent() {
  try {
    if (state.view === 'series') {
      const series = state.series;
      if (!series) return;
      if (inShelf(series.id)) {
        await invoke('remove_shelf', { id: series.id });
        toast('已移出书架');
      } else {
        const episode = series.episodes[0];
        await invoke('add_shelf', {
          entry: {
            id: series.id,
            title: series.title,
            author: '',
            cover: series.cover,
            kind: 'series',
            item_id: episode?.vid || '',
            chapter_title: episode ? `第 ${episode.index} 集` : `共 ${series.episodes.length} 集`,
            index: episode ? Math.max(0, episode.index - 1) : 0,
          },
        });
        toast('已加入书架');
      }
    } else {
      const book = state.book;
      if (!book) return;
      if (inShelf(book.id)) {
        await invoke('remove_shelf', { id: book.id });
        toast('已移出书架');
      } else {
        const chapter = state.chapters[state.chapterIndex];
        await invoke('add_shelf', {
          entry: {
            id: book.id,
            title: book.title,
            author: book.author,
            cover: book.cover,
            kind: 'book',
            item_id: chapter?.item_id || '',
            chapter_title: chapter?.title || '',
            index: state.chapterIndex,
          },
        });
        toast('已加入书架');
      }
    }
    state.shelf = await invoke('load_shelf');
    render({ keepScroll: true });
  } catch (err) {
    toast(errorText(err));
  }
}

/* ---- rendering ----------------------------------------------------------- */

function renderTabs() {
  $('#tabs').innerHTML = TABS.map((tab) => `
    <button class="tab ${state.tab === tab.id ? 'active' : ''}" data-tab="${tab.id}">
      <span class="tab-icon">${ICONS[tab.icon]}</span><span>${tab.label}</span>
    </button>`).join('');
}

function render(opts = {}) {
  const content = $('#content');
  const keep = opts.keepScroll ? content.scrollTop : 0;
  if (!opts.keepScroll) {
    state.userScrolled = false;
    state.lastAutoPage = 0;
  }

  renderTabs();
  $('#title').textContent = TITLES[state.view] || '茄阅';
  $('#back').classList.toggle('hidden', !canBack());

  let html = '';
  switch (state.view) {
    case 'home': html = renderHome(); break;
    case 'results': html = renderResults(); break;
    case 'library': html = renderLibrary(); break;
    case 'rank': html = renderRank(); break;
    case 'category': html = renderCategory(); break;
    case 'book': html = renderBook(); break;
    case 'series': html = renderSeries(); break;
    case 'reader': html = renderReader(); break;
    case 'manga': html = renderManga(); break;
    default: html = loadingScreen('加载中…');
  }
  content.innerHTML = html;
  if (state.view === 'reader' && opts.keepScroll) content.scrollTop = state.readerScroll;
  else content.scrollTop = keep;

  updateBusy();
}

function cardHitAttrs(hit) {
  return `data-action="open-hit" data-id="${esc(hit.id)}" data-series="${esc(hit.series || '')}" data-cover="${esc(hit.cover || '')}"`;
}

function homeCardHtml(hit) {
  return `<div class="card" tabindex="0" ${cardHitAttrs(hit)}>
    ${coverHtml(hit.cover, 'cover', hit.title)}
    <div class="card-title">${esc(hit.title)}</div>
    ${hit.author ? `<div class="card-author">${esc(hit.author)}</div>` : ''}
  </div>`;
}

function resultRowHtml(hit) {
  const tag = hit.series ? '短剧 · ' : '';
  return `<div class="row" tabindex="0" ${cardHitAttrs(hit)}>
    ${coverHtml(hit.cover, 'row-cover', hit.title)}
    <div class="row-body">
      <div class="row-title">${esc(tag + hit.title)}</div>
      ${hit.author ? `<div class="row-meta">${esc(hit.author)}</div>` : ''}
      ${hit.summary ? `<div class="row-summary">${esc(hit.summary)}</div>` : ''}
    </div>
  </div>`;
}

function rankRowHtml(hit, index) {
  return `<div class="rank-row" ${cardHitAttrs(hit)}>
    <div class="rank-num ${index < 3 ? 'top' : ''}">${index + 1}</div>
    ${coverHtml(hit.cover, 'row-cover', hit.title)}
    <div class="row-body">
      <div class="row-title">${esc(hit.title)}</div>
      ${hit.author ? `<div class="row-meta">${esc(hit.author)}</div>` : ''}
    </div>
  </div>`;
}

function commentHtml(comment) {
  return `<div class="comment">
    <div class="comment-head">
      <span class="comment-user">${esc(comment.user)}</span>
      <span class="comment-when">${esc(comment.when)}</span>
    </div>
    <div class="comment-text">${esc(comment.text)}</div>
    <div class="comment-meta">赞 ${esc(comment.digg)} · 回复 ${esc(comment.replies)}</div>
  </div>`;
}

function updateHomeMoreWrap() {
  const wrap = document.getElementById('home-more-wrap');
  if (!wrap) return;
  const index = state.home?.sections.findIndex((section) => section.next_offset != null) ?? -1;
  wrap.innerHTML = index >= 0
    ? `<button data-action="home-more" data-section="${index}">加载更多</button>`
    : '';
}

function updateSectionMoreButton(section, index) {
  const holder = document.getElementById(`home-more-${index}`);
  if (!holder || !section) return;
  const isRank = section.title.includes('排行榜');
  const limit = isRank ? Math.min(10, section.hits.length) : Math.min(section._visible || 60, section.hits.length);
  holder.innerHTML = !isRank && section.hits.length > limit
    ? `<button class="ghost" data-action="show-more-section" data-section="${index}">显示更多</button>`
    : '';
}

function updateResultsMore() {
  const wrap = document.getElementById('results-more-wrap');
  if (!wrap) return;
  wrap.innerHTML = state.searchNext != null
    ? '<button data-action="search-more">加载更多</button>'
    : '';
}

function updateRankFooter() {
  const footer = document.getElementById('rank-footer');
  if (!footer) return;
  footer.innerHTML = state.rankHits.length
    ? (state.rankMore
      ? '<div class="more-wrap"><button data-action="rank-more">加载更多</button></div>'
      : '<div class="empty" style="padding:20px">到底了</div>')
    : '<div class="empty">还没有榜单数据</div>';
}

function updateCommentsMore() {
  const count = document.getElementById('comments-count');
  if (count) count.textContent = `${state.commentsNext != null ? '已加载 ' : '共 '}${state.comments.length}`;
  const wrap = document.getElementById('comments-more-wrap');
  if (wrap) wrap.innerHTML = state.commentsNext != null
    ? '<button data-action="comments-more">加载更多评论</button>'
    : '';
}

function renderHome() {
  if (!state.home) return loadingScreen('加载首页…');
  const channels = state.home.tabs.map((tab) => `
    <button class="channel ${tab.index === state.homeTabIndex ? 'active' : ''}"
      data-action="channel" data-tab-type="${tab.tab_type}" data-index="${tab.index}"
      data-template="${esc(tab.client_template)}">${esc(tab.title)}</button>`).join('');

  const sections = state.home.sections.map(renderHomeSection).join('');
  const more = `<div class="more-wrap" id="home-more-wrap">${homeMoreButton()}</div>`;
  return `<div class="channels">${channels}</div>${sections || '<div class="empty">这个频道暂时没有内容</div>'}${more}`;
}

function renderHomeSection(section, index) {
  const isRank = section.title.includes('排行榜');
  const limit = isRank ? Math.min(10, section.hits.length) : Math.min(section._visible || 60, section.hits.length);
  const cards = section.hits.slice(0, limit).map(homeCardHtml).join('');
  const link = isRank
    ? '<span class="section-link" data-action="open-rank">查看完整榜单 〉</span>'
    : '';
  const showMore = !isRank && section.hits.length > limit
    ? `<button class="ghost" data-action="show-more-section" data-section="${index}">显示更多</button>`
    : '';
  return `<section class="section">
    <div class="section-head"><span class="section-title">${esc(section.title)}</span>${link}<span id="home-more-${index}">${showMore}</span></div>
    <div class="grid" id="home-grid-${index}">${cards}</div>
  </section>`;
}

function homeMoreButton() {
  const index = state.home?.sections.findIndex((section) => section.next_offset != null) ?? -1;
  if (index < 0) return '';
  return `<div class="more-wrap"><button data-action="home-more" data-section="${index}">加载更多</button></div>`;
}

function renderResults() {
  if (!state.hits.length) return '<div class="empty">没有结果，换个关键词试试</div>';
  const rows = state.hits.map(resultRowHtml).join('');
  const more = state.searchNext != null
    ? '<button data-action="search-more">加载更多</button>'
    : '';
  return `<div class="list" id="results-list">${rows}</div><div class="more-wrap" id="results-more-wrap">${more}</div>`;
}

function renderLibrary() {
  if (!state.shelf.length) {
    return '<div class="empty">书架还是空的，在详情页点「加入书架」</div>';
  }
  const cards = state.shelf.map((entry) => `
    <div class="card" tabindex="0" data-action="open-shelf" data-id="${esc(entry.id)}" data-kind="${esc(entry.kind)}">
      ${coverHtml(entry.cover, 'cover', entry.title)}
      <div class="card-title">${esc(entry.title)}</div>
      <div class="card-author">${esc(entry.chapter_title)}</div>
      <button class="ghost" data-action="shelf-remove" data-id="${esc(entry.id)}">移出</button>
    </div>`).join('');
  return `<div class="grid">${cards}</div>`;
}

function renderRank() {
  if (!state.rank) return loadingScreen('加载榜单…');
  const boards = state.rank.boards.map((board) => `
    <button class="channel ${board.algo === state.rankAlgo ? 'active' : ''}"
      data-action="rank-board" data-algo="${esc(board.algo)}">${esc(board.name)}</button>`).join('');
  const categories = state.rank.categories.map((category) => `
    <button class="channel ${category.id === state.rankCategory ? 'active' : ''}"
      data-action="rank-cat" data-id="${esc(category.id)}">${esc(category.name)}</button>`).join('');
  const rows = state.rankHits.map(rankRowHtml).join('');
  const footer = state.rankHits.length
    ? (state.rankMore
      ? '<div class="more-wrap"><button data-action="rank-more">加载更多</button></div>'
      : '<div class="empty" style="padding:20px">到底了</div>')
    : '<div class="empty">还没有榜单数据</div>';
  return `<div class="channels">${boards}</div>
    <div class="channels" style="border-bottom:0;padding-top:4px">${categories}</div>
    <div class="list" id="rank-list">${rows}</div><div id="rank-footer">${footer}</div>`;
}

function renderCategory() {
  if (!state.category) return loadingScreen('加载分类…');
  const tabs = state.category.tabs.map((tab) => {
    const value = Number(tab.value) || 1;
    return `<button class="channel ${value === state.categoryTab ? 'active' : ''}"
      data-action="category-tab" data-value="${value}">${esc(tab.name)}</button>`;
  }).join('');

  const groups = [];
  const known = new Set(state.category.dims.map((dim) => dim.value));
  const rest = state.category.tiles.filter((tile) => !known.has(tile.dim) || tile.dim === '0');
  if (rest.length) groups.push(['全部', rest]);
  for (const dim of state.category.dims) {
    if (dim.value === '0') continue;
    const tiles = state.category.tiles.filter((tile) => tile.dim === dim.value);
    if (tiles.length) groups.push([dim.name, tiles]);
  }

  const body = groups.map(([name, tiles]) => `
    <div class="dim">
      <div class="dim-title">${esc(name)}</div>
      <div class="tags">${tiles.map((tile) => `
        <button class="tag" data-action="tag" data-name="${esc(tile.name)}">${tile.hot ? '🔥 ' : ''}${esc(tile.name)}</button>`).join('')}</div>
    </div>`).join('');

  return `<div class="channels">${tabs}</div>${body}`;
}

function renderBook() {
  if (!state.book) return loadingScreen('加载目录…');
  const book = state.book;
  const chapters = state.chapters.map((chapter, index) => `
    <div class="chapter ${index === state.chapterIndex ? 'active' : ''}"
      data-action="chapter" data-index="${index}" title="${esc(chapter.title)}">${esc(chapter.title)}</div>`).join('');
  const own = inShelf(book.id);
  return `<div class="book-layout">
    <article>
      <div class="book-hero">
        ${coverHtml(book.cover, 'book-cover', book.title)}
        <div>
          <h2 class="book-title">${esc(book.title)}</h2>
          <div class="book-meta">${esc(book.author)}${book.is_comic ? ' · 漫画' : ''}</div>
          <div class="book-actions">
            <button class="primary" data-action="start-reading">${book.is_comic ? '从第一话开始看' : '从第一章开始读'}</button>
            <button data-action="toggle-shelf">${own ? '已在书架' : '加入书架'}</button>
            ${book.is_comic ? '' : '<button data-action="play-audio">听书（本章）</button>'}
          </div>
        </div>
      </div>
      <div class="summary">${esc(book.summary)}</div>
      ${renderComments()}
    </article>
    <aside class="book-side">
      <h3>目录</h3>
      <div class="chapter-list">${chapters}</div>
    </aside>
  </div>`;
}

function renderComments() {
  const rows = state.comments.map(commentHtml).join('');
  const more = state.commentsNext != null
    ? '<button data-action="comments-more">加载更多评论</button>'
    : '';
  return `<div class="divider"></div>
    <div class="comments-head"><h3>评论</h3><span id="comments-count">${state.commentsNext != null ? '已加载 ' : '共 '}${state.comments.length}</span></div>
    <div id="comments-list">${rows || '<div class="empty" style="padding:16px">还没有评论</div>'}</div>
    <div class="more-wrap" id="comments-more-wrap">${more}</div>`;
}

function renderSeries() {
  if (!state.series) return loadingScreen('加载剧集…');
  const series = state.series;
  const own = inShelf(series.id);
  const episodes = series.episodes.map((episode) => `
    <button class="episode" data-action="play-episode" data-vid="${esc(episode.vid)}"
      data-title="${esc(episode.title)}" data-index="${episode.index}">
      <span class="episode-index">第 ${episode.index} 集</span>
      <span>${esc(episode.title || '')}</span>
      <span class="row-meta" style="margin-left:auto">${esc(episode.duration)}s</span>
    </button>`).join('');
  return `<div class="book-hero">
      ${coverHtml(series.cover, 'book-cover', series.title)}
      <div>
        <h2 class="book-title">${esc(series.title)}</h2>
        <div class="book-meta">共 ${series.episodes.length} 集</div>
        <div class="book-actions"><button data-action="toggle-shelf">${own ? '已在书架' : '加入书架'}</button></div>
        <div class="summary">${esc(series.intro)}</div>
      </div>
    </div>
    <div class="divider"></div>${episodes}`;
}

function renderReader() {
  if (!state.chapterTitle && !state.paragraphs.length) return loadingScreen('加载章节…');
  const body = state.paragraphs.map((paragraph) => `<p>${esc(paragraph)}</p>`).join('');
  const own = state.book ? inShelf(state.book.id) : false;
  const chapter = state.chapters[state.chapterIndex];
  return `<div class="reader-page">
      <h2 class="reader-title">${esc(state.chapterTitle)}</h2>${body}
    </div>
    <div class="reader-bar">
      <button data-action="prev-chapter" ${state.chapterIndex <= 0 ? 'disabled' : ''}>上一章</button>
      <button data-action="next-chapter" ${state.chapterIndex + 1 >= state.chapters.length ? 'disabled' : ''}>下一章</button>
      <button data-action="open-directory">目录</button>
      <button data-action="toggle-shelf">${own ? '已在书架' : '加入书架'}</button>
      <button data-action="play-audio">听书</button>
      <span class="spacer"></span>
      <span class="chapter-name">${esc(chapter?.title || state.chapterTitle)}</span>
    </div>`;
}

function renderManga() {
  if (!state.mangaPages.length) return loadingScreen('解密漫画…');
  const images = state.mangaPages.map((src) => `<img src="${esc(src)}" loading="lazy" alt="">`).join('');
  const chapter = state.chapters[state.chapterIndex];
  return `<div class="manga-page">${images}</div>
    <div class="reader-bar">
      <button data-action="prev-chapter" ${state.chapterIndex <= 0 ? 'disabled' : ''}>上一话</button>
      <button data-action="next-chapter" ${state.chapterIndex + 1 >= state.chapters.length ? 'disabled' : ''}>下一话</button>
      <button data-action="open-directory">目录</button>
      <span class="spacer"></span>
      <span class="chapter-name">${esc(chapter?.title || '')} · ${state.mangaPages.length} 页</span>
    </div>`;
}

/* ---- auto paging --------------------------------------------------------- */

function maybeLoadMore() {
  const content = $('#content');
  if (!content) return;
  if (state.view === 'reader' || state.view === 'manga') return;
  if (!state.userScrolled) return;
  if (Date.now() - state.lastAutoPage < 600) return;
  const nearBottom = content.scrollTop + content.clientHeight >= content.scrollHeight - 320;
  if (!nearBottom) return;
  state.lastAutoPage = Date.now();

  if (state.view === 'home') {
    const index = state.home?.sections.findIndex((section) => section.next_offset != null) ?? -1;
    if (index >= 0) loadHomeMore(index);
  } else if (state.view === 'results') {
    if (state.searchNext != null) loadSearchMore();
  } else if (state.view === 'rank') {
    loadRankMore();
  }
}

/* ---- events -------------------------------------------------------------- */

function bindEvents() {
  $('#tabs').addEventListener('click', (event) => {
    const button = event.target.closest('[data-tab]');
    if (button) selectTab(button.dataset.tab);
  });

  $('#search-form').addEventListener('submit', (event) => {
    event.preventDefault();
    submitSearch($('#search-input').value);
  });

  $('#back').addEventListener('click', back);

  const content = $('#content');
  content.addEventListener('scroll', () => {
    if (state.view === 'reader') state.readerScroll = content.scrollTop;
    if (content.scrollTop > 4) state.userScrolled = true;
    if (scrollScheduled) return;
    scrollScheduled = true;
    requestAnimationFrame(() => {
      scrollScheduled = false;
      maybeLoadMore();
    });
  });

  content.addEventListener('click', (event) => {
    const element = event.target.closest('[data-action]');
    if (!element || !content.contains(element)) return;
    const action = element.dataset.action;
    switch (action) {
      case 'open-hit':
        if (element.dataset.series) openSeries(element.dataset.series, element.dataset.cover);
        else openBook(element.dataset.id);
        break;
      case 'open-shelf':
        if (element.dataset.kind === 'series') openSeries(element.dataset.id);
        else {
          const entry = state.shelf.find((row) => row.id === element.dataset.id);
          openBook(element.dataset.id, entry?.item_id || null);
        }
        break;
      case 'shelf-remove':
        event.stopPropagation();
        invoke('remove_shelf', { id: element.dataset.id })
          .then(() => invoke('load_shelf'))
          .then((shelf) => { state.shelf = shelf; render({ keepScroll: true }); })
          .catch((err) => toast(errorText(err)));
        break;
      case 'channel':
        loadHomeChannel(Number(element.dataset.tabType), Number(element.dataset.index), element.dataset.template);
        break;
      case 'home-more':
        loadHomeMore(Number(element.dataset.section));
        break;
      case 'show-more-section': {
        const section = state.home?.sections[Number(element.dataset.section)];
        if (section) {
          section._visible = (section._visible || 60) + 60;
          render({ keepScroll: true });
        }
        break;
      }
      case 'open-rank':
        selectTab('rank');
        break;
      case 'search-more':
        loadSearchMore();
        break;
      case 'rank-board':
        state.rankAlgo = element.dataset.algo;
        state.rankHits = [];
        state.rankMore = true;
        loadRankPage(0);
        render({ keepScroll: true });
        break;
      case 'rank-cat':
        state.rankCategory = element.dataset.id;
        state.rankHits = [];
        state.rankMore = true;
        loadRankPage(0);
        render({ keepScroll: true });
        break;
      case 'rank-more':
        loadRankMore();
        break;
      case 'category-tab':
        loadCategory(Number(element.dataset.value));
        break;
      case 'tag':
        submitSearch(element.dataset.name);
        break;
      case 'chapter':
        openChapter(Number(element.dataset.index));
        break;
      case 'start-reading':
        openChapter(0);
        break;
      case 'toggle-shelf':
        toggleShelfCurrent();
        break;
      case 'play-audio':
        playAudio();
        break;
      case 'play-episode':
        playEpisode(element.dataset.vid, element.dataset.title, Number(element.dataset.index));
        break;
      case 'prev-chapter':
        openChapter(state.chapterIndex - 1);
        break;
      case 'next-chapter':
        openChapter(state.chapterIndex + 1);
        break;
      case 'open-directory':
        openDirectory();
        break;
      case 'comments-more':
        if (state.commentsNext != null) loadComments(state.commentsNext);
        break;
      default:
        break;
    }
  });

  content.addEventListener('keydown', (event) => {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    const card = event.target.closest('[data-action="open-hit"], [data-action="open-shelf"]');
    if (!card) return;
    event.preventDefault();
    card.click();
  });
}

/* ---- boot ---------------------------------------------------------------- */

async function boot() {
  window.addEventListener('error', (event) => {
    logDebug(`error: ${event.message} @ ${event.filename}:${event.lineno}`);
  });
  window.addEventListener('unhandledrejection', (event) => {
    logDebug(`rejection: ${errorText(event.reason)}`);
  });
  bindEvents();
  renderTabs();
  setStatus('连接后端…');
  render();
  logDebug('boot');
  try {
    await invoke('backend_ready');
    logDebug('backend ready');
  } catch (err) {
    $('#content').innerHTML = `<div class="empty">后端初始化失败：${esc(errorText(err))}</div>`;
    setStatus('后端不可用');
    return;
  }
  await refreshShelf();
  render();
  loadTab('home');
  logDebug('home requested');
}

boot();
