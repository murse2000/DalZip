import './style.css';
import { initializeUpdater } from './updater';
import { getVersion } from '@tauri-apps/api/app';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open, save } from '@tauri-apps/plugin-dialog';
import { createIcons, FolderOpen, Archive, Plus, Settings2, ArrowUpRight, ArrowDownToLine, ChevronRight, Search, File, Folder, X, ShieldCheck, Zap, Check, Command, Moon, ArrowLeft, ExternalLink, LockKeyhole, LoaderCircle } from 'lucide';
const icons = { FolderOpen, Archive, Plus, Settings2, ArrowUpRight, ArrowDownToLine, ChevronRight, Search, File, Folder, X, ShieldCheck, Zap, Check, Command, Moon, ArrowLeft, ExternalLink, LockKeyhole, LoaderCircle };
const app = document.querySelector<HTMLDivElement>('#app')!;
const native = isTauri();
type Entry = { name: string; size: number; compressed: number; directory: boolean; encrypted: boolean };
type Info = { format: string; sizeKnown: boolean; path: string; entries: Entry[]; totalSize: number; archiveSize: number };
type Outcome = { warning: string | null; path: string; bytes: number; seconds: number; files: number };
type Progress = { completed: number; total: number; elapsed: number; file: string };
type Recent = { path: string; name: string; date: string };
let page: 'home' | 'archive' | 'create' | 'settings' = 'home';
let info: Info | null = null, sources: string[] = [], busy = false, loading = false, os = 'macos';
let appVersion = '0.1.1', updating = false;
let checkForUpdates: ((manual?: boolean) => Promise<void>) | undefined;
let jobTitle = '', progress: Progress | null = null, outcome: Outcome | null = null;
const extensions = ['zip','zipx','7z','rar','tar','gz','tgz','bz2','tbz2','xz','txz','zst'];
let format = 'zip';
let archivePassword: string | null = null;
let openAfter = localStorage.getItem('dalzip-open-after') !== 'false';
let trashAfter = localStorage.getItem('dalzip-trash-after') === 'true';
let query = '', offset = 0, level = 1, toastTimer: ReturnType<typeof setTimeout>;
let recent: Recent[] = [];
try { const saved = JSON.parse(localStorage.getItem('dalzip-recent') || '[]'); if (Array.isArray(saved)) recent = saved.filter(r => typeof r.path === 'string' && typeof r.name === 'string').slice(0, 8); } catch { /* 손상된 최근 기록은 빈 목록으로 시작합니다. */ }
let limit = Number(localStorage.getItem('dalzip-limit') || 20);
if (!Number.isInteger(limit) || limit < 1 || limit > 2048) limit = 20;
const e = (s: string) => s.replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]!));
const name = (p: string) => p.split(/[\\/]/).pop() || p;
const icon = (n: string, cls = '') => `<i data-lucide="${n}" class="${cls}"></i>`;
const bytes = (n: number) => { if (!n) return '0 B'; const i = Math.min(Math.floor(Math.log(n) / Math.log(1024)), 4); return `${(n / 1024 ** i).toLocaleString('ko-KR', { maximumFractionDigits: i ? 1 : 0 })} ${['B','KB','MB','GB','TB'][i]}`; };
const command = () => os === 'macos' ? '⌘' : 'Ctrl';
function drawIcons() { createIcons({ icons, attrs: { 'stroke-width': 1.7 } }); }
function notify(message: string, error = false) {
    clearTimeout(toastTimer); const box = document.querySelector<HTMLElement>('#toast')!;
    box.textContent = message; box.className = `toast visible ${error ? 'error' : ''}`;
    toastTimer = setTimeout(() => box.classList.remove('visible'), error ? 12000 : 6500);
}
function ready() { if (!native) { notify('실제 파일 작업은 DalZip 데스크톱 앱에서 사용할 수 있습니다. 웹 화면은 디자인 미리보기입니다.'); return false; } return true; }
function remember(path: string) {
    recent = [{ path, name: name(path), date: new Date().toLocaleDateString('ko-KR') }, ...recent.filter(r => r.path !== path)].slice(0, 8);
    localStorage.setItem('dalzip-recent', JSON.stringify(recent));
}
function render() {
    app.innerHTML = `<div class="ambient ambient-one"></div><div class="ambient ambient-two"></div><div class="shell">
    <aside class="sidebar glass"><a class="brand" href="#" aria-label="DalZip 홈"><img src="/dalzip-icon.png" alt="달베어 ZIP 아이콘"><span>DalZip<small>by dalbear</small></span></a>
    <div class="nav-label">WORKSPACE</div><nav>
    <button data-page="home" class="nav ${page === 'home' ? 'active' : ''}">${icon('Archive')}<span>시작하기</span></button>
    <button id="nav-open" class="nav ${page === 'archive' ? 'active' : ''}">${icon('FolderOpen')}<span>압축 파일 열기</span></button>
    <button data-page="create" class="nav ${page === 'create' ? 'active' : ''}">${icon('Plus')}<span>새로 압축하기</span></button></nav>
    <div class="nav-label recent-label">RECENT <span>${recent.length.toString().padStart(2, '0')}</span></div>
    <div class="sidebar-recents">${recent.length ? recent.slice(0, 5).map((r, i) => `<button class="recent-nav" data-recent="${i}" title="${e(r.path)}">${icon('Archive')}<span>${e(r.name)}</span></button>`).join('') : '<p class="muted small">열어 본 압축 파일이<br>여기에 표시됩니다.</p>'}</div>
    <div class="sidebar-bottom"><div class="local-note">${icon('ShieldCheck')}<span>파일은 이 기기에만</span><span class="status-dot"></span></div><button data-page="settings" class="nav ${page === 'settings' ? 'active' : ''}">${icon('Settings2')}<span>설정 및 파일 연결</span></button><div class="signature">DALBEAR SOFTWARE <span>v0.1</span></div></div></aside>
    <main><header class="topbar"><div class="breadcrumb">DalZip ${icon('ChevronRight')} <span>${{home:'시작하기',archive:'압축 파일',create:'새로 압축하기',settings:'설정'}[page]}</span></div><div class="top-right"><span class="pill">${icon('Moon')} Made for your files</span><button id="quick-open" class="icon-button" title="압축 파일 열기">${icon('FolderOpen')}</button></div></header>
    <section class="page">${page === 'home' ? home() : page === 'archive' ? archiveView() : page === 'create' ? createView() : settingsView()}</section>
    <footer><span><b class="status-dot"></b>${native ? '로컬에서 안전하게 처리합니다' : '브라우저 디자인 미리보기'}</span><span>ZIP · 7Z · RAR · TAR <b class="footer-dot">·</b> ${os === 'macos' ? 'Apple Silicon' : 'Windows'}</span></footer></main></div><div id="job"></div><div id="toast" class="toast" role="status" aria-live="polite"></div><div id="drop-overlay">${icon('ArrowDownToLine')}<h2>이곳에 놓아주세요</h2><p>압축 파일은 열고, 다른 파일은 새로 압축합니다.</p></div>`;
    bind(); renderJob(); drawIcons();
}
function home() { return `<div class="welcome"><div class="eyebrow"><span></span> LESS WEIGHT. MORE SPACE.</div><h1>가볍게 담고,<br>빠르게 펼치세요<span>.</span></h1><p>복잡한 파일도, 달베어와 함께 간결하게.<br>당신의 파일을 위한 작은 압축 공간.</p><img class="hero-icon" src="/dalzip-icon.png" alt="초승달 곰과 지퍼 폴더"></div>
    <div class="action-grid"><button class="action-card glass primary-card" id="open-main"><div class="action-symbol">${icon('FolderOpen')}</div><span class="action-shortcut">${command()} O</span><h2>압축 파일 열기</h2><p>다양한 압축 파일을 확인하고 안전하게 꺼내세요.</p><div class="card-bottom"><span>압축 파일 선택</span>${icon('ArrowUpRight')}</div></button>
    <button class="action-card glass" id="create-main"><div class="action-symbol violet">${icon('Plus')}</div><span class="action-shortcut">${command()} N</span><h2>새로 압축하기</h2><p>ZIP, 7Z, TAR.GZ로 파일과 폴더를 모아보세요.</p><div class="card-bottom"><span>파일 또는 폴더 선택</span>${icon('ArrowUpRight')}</div></button></div>
    <div class="drop-hint">${icon('ArrowDownToLine')}<span>파일을 이 창에 끌어다 놓아도 좋아요</span><span class="hint-line"></span><small>DRAG & DROP</small></div>
    <div class="section-heading"><h3>최근 압축 파일</h3>${recent.length ? '<button id="clear-recent" class="text-button">기록 지우기</button>' : '<span>이 기기의 작업 기록</span>'}</div>
    <div class="recent-list">${recent.length ? recent.slice(0, 3).map((r, i) => `<button data-recent="${i}" class="recent-row"><div class="file-symbol">${icon('Archive')}</div><div><strong>${e(r.name)}</strong><small>${e(r.path)}</small></div><time>${e(r.date)}</time>${icon('ChevronRight')}</button>`).join('') : `<div class="empty-recent">${icon('Archive')}<div><strong>아직 열어 본 압축 파일이 없어요</strong><p>첫 압축 파일을 열어 작업을 시작하세요.</p></div><button id="empty-open" class="text-button">파일 열기 ${icon('ArrowUpRight')}</button></div>`}</div>
    <div class="feature-strip"><span>${icon('Zap')} 병렬 압축 해제</span><span>${icon('ShieldCheck')} 파일 무결성 확인</span><span>${icon('LockKeyhole')} 클라우드 전송 없음</span></div>`; }
function archiveView() {
    if (!info) return '<div class="empty-state"><h1>압축 파일을 열어주세요</h1><button id="empty-open" class="button primary">파일 선택</button></div>';
    return `<div class="page-heading"><div class="eyebrow">ARCHIVE EXPLORER</div><h1 title="${e(info.path)}">${e(name(info.path))}</h1><p class="truncate">${e(info.path)}</p></div><div class="archive-summary glass"><div><small>항목</small><strong>${info.entries.length.toLocaleString()}<em>개</em></strong></div><div><small>압축 크기</small><strong>${bytes(info.archiveSize)}</strong></div><div><small>해제 후 크기</small><strong>${info.sizeKnown ? bytes(info.totalSize) : '해제 시 계산'}</strong></div><button id="extract" class="button primary" ${busy ? 'disabled' : ''}>${icon('ArrowDownToLine')} 모두 압축 해제</button></div>
    <div class="table-toolbar"><label class="search">${icon('Search')}<input id="search" placeholder="이름 또는 경로 검색" value="${e(query)}" aria-label="압축 파일 내부 검색"></label><span id="entry-count"></span></div><div class="file-table"><div class="table-head"><span>이름 / 경로</span><span>원본 크기</span><span>압축 크기</span></div><div id="entries"></div></div><div class="pagination"><button id="previous" class="text-button">${icon('ArrowLeft')} 이전</button><span id="page-number"></span><button id="next" class="text-button">다음 ${icon('ChevronRight')}</button></div><p class="small muted safety-note">${icon('ShieldCheck')} 선택한 위치에 새 폴더를 만듭니다. 기존 파일은 덮어쓰지 않습니다.</p>`;
}
function renderEntries() {
    if (page !== 'archive' || !info) return;
    const filtered = info.entries.filter(v => v.name.toLocaleLowerCase().includes(query.toLocaleLowerCase()));
    if (offset >= filtered.length) offset = 0;
    document.querySelector('#entries')!.innerHTML = filtered.slice(offset, offset + 150).map(v => `<div class="file-row"><span title="${e(v.name)}">${icon(v.directory ? 'Folder' : 'File', v.directory ? 'folder-color' : '')}<b>${e(v.name)}</b>${v.encrypted ? icon('LockKeyhole', 'lock') : ''}</span><span>${v.directory ? '—' : bytes(v.size)}</span><span>${v.directory || !v.compressed ? '—' : bytes(v.compressed)}</span></div>`).join('') || '<div class="no-results">일치하는 항목이 없습니다.</div>';
    document.querySelector('#entry-count')!.textContent = `${filtered.length.toLocaleString()}개 항목`;
    document.querySelector('#page-number')!.textContent = `${Math.floor(offset / 150) + 1} / ${Math.max(1, Math.ceil(filtered.length / 150))}`;
    (document.querySelector('#previous') as HTMLButtonElement).disabled = offset === 0;
    (document.querySelector('#next') as HTMLButtonElement).disabled = offset + 150 >= filtered.length;
    drawIcons();
}
function createView() { return `<div class="page-heading"><div class="eyebrow">CREATE AN ARCHIVE</div><h1>하나로 모아, 더 가볍게.</h1><p>압축할 파일과 폴더를 추가하세요.</p></div><div class="source-actions"><button id="add-files" class="button">${icon('Plus')} 파일 추가</button><button id="add-folder" class="button">${icon('FolderOpen')} 폴더 추가</button><span>${sources.length}개 선택</span></div><div class="source-list glass">${sources.length ? sources.map((s, i) => `<div class="source-row">${icon('File')}<div><strong>${e(name(s))}</strong><small>${e(s)}</small></div><button data-remove="${i}" class="icon-button" aria-label="${e(name(s))} 제거">${icon('X')}</button></div>`).join('') : `<div class="source-empty">${icon('Archive')}<h3>어떤 파일을 담을까요?</h3><p>위 버튼으로 추가하거나 파일을 끌어다 놓으세요.</p></div>`}</div><div class="compression-options"><label>압축 형식<select id="format">${["zip", "7z", "tar.gz"].map(f => `<option value="${f}" ${format === f ? "selected" : ""}>${f.toUpperCase()}</option>`).join("")}</select></label><label>압축 수준<select id="level"><option value="0" ${level === 0 ? 'selected' : ''}>저장만 · 압축 안 함</option><option value="1" ${level === 1 ? 'selected' : ''}>빠르게 · 속도 우선</option><option value="6" ${level === 6 ? 'selected' : ''}>균형 있게 · 표준 압축</option><option value="9" ${level === 9 ? 'selected' : ''}>작게 · 높은 압축률</option></select></label><button id="compress" class="button primary" ${!sources.length || busy ? 'disabled' : ''}>${icon('Archive')} ${format.toUpperCase()}으로 압축</button></div><label class="option-toggle"><input type="checkbox" id="trash-after-job" ${trashAfter ? "checked" : ""}> 압축 완료·내용 검증 후 원본을 휴지통으로 이동</label><div class="info-panel">${icon('ShieldCheck')}<p>완성된 압축 파일만 저장합니다. 원본 정리를 켜면 해제 검증과 원본 비교를 추가로 진행합니다.</p></div>`; }
function settingsView() { return `<div class="page-heading"><div class="eyebrow">PREFERENCES</div><h1>나에게 맞는 작은 설정.</h1><p>파일 연결과 안전한 압축 해제를 관리하세요.</p></div><div class="settings-card glass"><div class="setting-title">${icon('Archive')}<div><h3>기본 압축 프로그램</h3><p>선택한 형식의 파일을 더블클릭하면 DalZip으로 열도록 설정합니다.</p></div></div><div class="association-row"><div class="extension">연결</div><div><strong>압축 파일 확장자</strong><p>사용할 확장자를 선택하세요.</p></div><button id="set-default" class="button primary">${os === 'macos' ? '기본 앱으로 설정' : 'Windows 설정에서 연결'} ${icon('ExternalLink')}</button></div><div class="extensions">${extensions.map(ext => `<label><input type="checkbox" name="association" value="${ext}" checked> .${ext}</label>`).join("")}</div><p class="small muted">${os === 'macos' ? '응용 프로그램 폴더에 DalZip을 설치한 뒤 설정하세요. Finder → 정보 가져오기 → 다음으로 열기에서도 변경할 수 있습니다.' : '설치 시 압축 파일 연결 후보로 등록합니다. 최종 기본 앱 선택은 Windows 설정에서 사용자가 직접 진행합니다.'}</p></div><div class="settings-card glass"><div class="setting-title">${icon('ShieldCheck')}<div><h3>압축 해제 안전 한도</h3><p>알려진 크기는 시작 전에, 스트림 형식은 해제 중에도 한도를 검사합니다.</p></div></div><label class="limit-label">최대 해제 크기 <input id="limit" type="number" min="1" max="2048" value="${limit}"> GiB <button id="save-limit" class="button">저장</button></label><p class="small muted">1~2048 GiB · 압축 파일당 최대 100,000개 항목 · 심볼릭 링크와 위험 경로 차단</p></div><div class="settings-card glass"><div class="setting-title">${icon('Settings2')}<div><h3>작업 완료 후</h3><p>파일 정리 방식을 선택하세요.</p></div></div><label class="option-toggle"><input id="open-after" type="checkbox" ${openAfter ? 'checked' : ''}> 압축 해제 후 결과 폴더 열기</label><label class="option-toggle"><input id="trash-after" type="checkbox" ${trashAfter ? 'checked' : ''}> 압축 완료·내용 검증 후 원본을 휴지통으로 이동</label><p class="small muted">원본 정리는 기본적으로 꺼져 있습니다. 검증에 실패하면 원본을 보존합니다.</p></div><div class="settings-card glass"><div class="setting-title">${icon('FolderOpen')}<div><h3>우클릭으로 압축하기</h3><p>선택 항목에 따라 ‘대표 항목명.zip으로 압축하기’ 메뉴를 표시합니다.</p></div></div><button id="shell-settings" class="button" style="margin-top:18px">${os === 'macos' ? 'Finder 확장 등록 및 설정 열기' : '탐색기 확장 등록'}</button><p class="small muted" style="margin-top:12px">${os === 'macos' ? '시스템 설정 → 일반 → 로그인 항목 및 확장 프로그램 → Finder에서 DalZip을 켜세요.' : 'Windows 11에서는 우클릭 → 추가 옵션 표시에서 메뉴를 확인하세요.'}</p></div><div class="settings-card glass"><div class="setting-title">${icon("ArrowDownToLine")}<div><h3>앱 업데이트</h3><p>새 버전을 알리고, 동의한 경우에만 다운로드·설치합니다.</p></div></div><button id="check-update" class="button" style="margin-top:18px">업데이트 확인</button></div><div class="about"><img src="/dalzip-icon.png" alt="DalZip"><div><strong>DalZip <span>${e(appVersion)}</span></strong><p>by dalbear · 당신의 파일을 위한 압축 공간</p><small>열기: ZIP · 7Z · RAR · TAR · GZ · BZ2 · XZ · ZST / 생성: ZIP · 7Z · TAR.GZ</small></div></div>`; }
function renderJob() {
    const box = document.querySelector('#job')!;
    if (updating) { box.innerHTML = ''; return; }
    if (!busy && !outcome && !loading) { box.innerHTML = ''; return; }
    const pct = progress?.total ? Math.min(100, progress.completed / progress.total * 100) : 0;
    box.innerHTML = `<div class="job-panel glass" role="status" aria-live="polite"><div class="job-heading">${icon(busy || loading ? 'LoaderCircle' : 'Check', busy || loading ? 'spinning' : 'success')}<strong>${loading ? '압축 파일을 읽고 있습니다' : busy ? jobTitle : '작업을 완료했습니다'}</strong>${outcome && !busy ? `<button id="dismiss-job" class="icon-button" aria-label="완료 알림 닫기">${icon('X')}</button>` : ''}</div>${busy ? `<div class="progress-track"><span style="width:${pct}%"></span></div><div class="progress-meta"><span>${bytes(progress?.completed || 0)} / ${progress?.total ? bytes(progress.total) : '준비 중'}</span><strong>${pct.toFixed(0)}%</strong></div><p class="job-file">${e(progress?.file || '파일을 확인하고 있습니다…')}</p><div class="job-bottom"><span>${progress?.elapsed ? `${bytes(progress.completed / progress.elapsed)}/s · ${progress.elapsed.toFixed(1)}초` : '준비 중'}</span><button id="cancel" class="text-button">작업 취소</button></div>` : outcome ? `<p class="job-file">${e(outcome.path)}</p><div class="job-bottom"><span>${outcome.files}개 · ${bytes(outcome.bytes)} · ${outcome.seconds.toFixed(2)}초</span><button id="reveal-result" class="text-button">폴더에서 보기 ${icon('ArrowUpRight')}</button></div>` : '<p class="small muted">ZIP의 파일 목록을 확인하고 있습니다.</p>'}</div>`;
    document.querySelector('#cancel')?.addEventListener('click', async () => { await invoke('cancel_job'); jobTitle = '취소 중 · 미완성 파일 정리'; renderJob(); });
    document.querySelector('#dismiss-job')?.addEventListener('click', () => { outcome = null; renderJob(); });
    document.querySelector('#reveal-result')?.addEventListener('click', () => { void invoke('reveal', {path: outcome!.path}).catch(error => notify(String(error), true)); });
    drawIcons();
}
async function loadArchive(path: string, password: string | null = null) {
    if (busy || loading || !ready()) return;
    loading = true; outcome = null; renderJob();
    try { info = await invoke<Info>('inspect_archive', {path, password, limitGib:limit}); archivePassword = password; query = ''; offset = 0; page = 'archive'; remember(path); }
    catch (error) { loading = false; renderJob(); if (String(error).includes('PASSWORD_REQUIRED')) { const value = await requestPassword(); if (value !== null) await loadArchive(path, value); return; } notify(`압축 파일을 열 수 없습니다: ${String(error)}`, true); return; }
    loading = false; render();
}
async function chooseArchive() { if (busy || loading || !ready()) return; try { const path = await open({ multiple: false, filters: [{name:'압축 파일', extensions}] }); if (typeof path === 'string') await loadArchive(path); } catch (error) { notify(String(error), true); } }
async function addSources(directory: boolean) {
    if (busy || !ready()) return;
    try { const paths = await open({ multiple: true, directory }); if (paths) { sources = [...new Set([...sources, ...(Array.isArray(paths) ? paths : [paths])])]; page = 'create'; render(); } } catch (error) { notify(String(error), true); }
}
async function runJob(title: string, commandName: string, args: Record<string, unknown>) {
    if (busy) return;
    busy = true; jobTitle = title; progress = null; outcome = null; render();
    try { outcome = await invoke<Outcome>(commandName, args); if (commandName === 'create_archive') { remember(outcome.path); if (args.trashAfter && !outcome.warning) sources = []; } }
    catch (error) { busy = false; render(); notify(String(error), true); return; }
    busy = false; render();
    if (outcome?.warning) notify(outcome.warning, true);
    if (commandName === 'extract_archive' && openAfter && outcome) await invoke('open_folder', {path:outcome.path}).catch(error => notify(String(error), true));
}
async function extract(destinationPath?: string) {
    if (!info || busy || !ready()) return;
    let password: string | null = archivePassword;
    if (password === null && info.entries.some(v => v.encrypted)) { password = await requestPassword(); if (password === null) return; }
    try { const destination = destinationPath ?? await open({directory:true, multiple:false, title:'압축을 해제할 상위 폴더 선택'}); if (typeof destination === 'string') await runJob('안전하게 압축 해제 중', 'extract_archive', {path:info.path, destination, password, limitGib:limit}); } catch (error) { notify(String(error), true); }
}
function requestPassword(): Promise<string | null> {
    return new Promise(resolve => {
        const dialog = document.createElement('dialog'); dialog.className = 'password-dialog';
        dialog.innerHTML = `<form method="dialog"><h2>암호화된 압축 파일입니다</h2><p>암호는 저장하지 않습니다.</p><input type="password" name="password" placeholder="압축 파일 암호" autocomplete="off" autofocus aria-label="압축 파일 암호"><div><button value="cancel" class="button">취소</button><button value="ok" class="button primary">계속</button></div></form>`;
        document.body.append(dialog); dialog.showModal(); dialog.addEventListener('close', () => { const value = (dialog.querySelector('input')!).value; resolve(dialog.returnValue === 'ok' ? value : null); dialog.remove(); }, {once:true});
    });
}
async function compress() {
    if (!sources.length || busy || !ready()) return;
    try { const suggested = await invoke<string>('suggest_output', {sources, format}); const output = await save({title:'새 압축 파일 저장', defaultPath:suggested, filters:[{name:format.toUpperCase()+' 압축 파일', extensions:[format === 'tar.gz' ? 'gz' : format]}]}); if (output) await runJob('파일을 하나로 압축 중', 'create_archive', { sources, output, level, format, trashAfter }); } catch (error) { notify(String(error), true); }
}
function bind() {
    document.querySelector('#check-update')?.addEventListener('click', () => { if (ready()) void checkForUpdates?.(true); });
    document.querySelector('.brand')?.addEventListener('click', ev => { ev.preventDefault(); page = 'home'; render(); });
    document.querySelectorAll<HTMLElement>('[data-page]').forEach(el => el.addEventListener('click', () => { page = el.dataset.page as typeof page; render(); }));
    for (const id of ['nav-open','quick-open','open-main','empty-open']) document.getElementById(id)?.addEventListener('click', () => { void chooseArchive(); });
    document.querySelector('#create-main')?.addEventListener('click', () => { page = 'create'; render(); });
    document.querySelectorAll<HTMLElement>('[data-recent]').forEach(el => el.addEventListener('click', () => { void loadArchive(recent[Number(el.dataset.recent)].path); }));
    document.querySelector('#clear-recent')?.addEventListener('click', () => { recent = []; localStorage.removeItem('dalzip-recent'); render(); });
    document.querySelector('#add-files')?.addEventListener('click', () => { void addSources(false); });
    document.querySelector('#add-folder')?.addEventListener('click', () => { void addSources(true); });
    document.querySelectorAll<HTMLElement>('[data-remove]').forEach(el => el.addEventListener('click', () => { if (busy) return; sources.splice(Number(el.dataset.remove), 1); render(); }));
    document.querySelector('#level')?.addEventListener('change', ev => { level = Number((ev.target as HTMLSelectElement).value); });
    document.querySelector('#compress')?.addEventListener('click', () => { void compress(); });
    document.querySelector('#extract')?.addEventListener('click', () => { void extract(); });
    document.querySelector('#search')?.addEventListener('input', ev => { query = (ev.target as HTMLInputElement).value; offset = 0; renderEntries(); });
    document.querySelector('#previous')?.addEventListener('click', () => { offset = Math.max(0, offset - 150); renderEntries(); });
    document.querySelector('#next')?.addEventListener('click', () => { offset += 150; renderEntries(); });
    document.querySelector('#set-default')?.addEventListener('click', async () => { if (!ready()) return; try { notify(await invoke<string>('set_default', {extensions:Array.from(document.querySelectorAll<HTMLInputElement>('input[name=association]:checked')).map(v => v.value)})); } catch (error) { notify(String(error), true); } });
    document.querySelector('#save-limit')?.addEventListener('click', () => { const value = Number((document.querySelector('#limit') as HTMLInputElement).value); if (!Number.isInteger(value) || value < 1 || value > 2048) { notify('1~2048 사이의 정수를 입력하세요.', true); return; } limit = value; localStorage.setItem('dalzip-limit', String(limit)); notify('해제 크기 한도를 저장했습니다.'); });
    document.querySelector('#format')?.addEventListener('change', ev => { format = (ev.target as HTMLSelectElement).value; render(); });
    document.querySelector('#open-after')?.addEventListener('change', ev => { openAfter = (ev.target as HTMLInputElement).checked; localStorage.setItem('dalzip-open-after', String(openAfter)); });
    for (const id of ['trash-after','trash-after-job']) document.getElementById(id)?.addEventListener('change', ev => { trashAfter = (ev.target as HTMLInputElement).checked; localStorage.setItem('dalzip-trash-after', String(trashAfter)); });
    document.querySelector('#shell-settings')?.addEventListener('click', async () => { if (!ready()) return; try { notify(await invoke<string>('shell_settings')); } catch (error) { notify(String(error), true); } });
    renderEntries();
}
async function handlePaths(paths: string[], compress = false, unpack = false) {
    if (!paths.length) return;
    if (busy || loading) { notify('진행 중인 작업을 마친 뒤 파일을 다시 열어주세요.'); return; }
    if (unpack) {
        for (const path of paths) {
            info = null;
            await loadArchive(path);
            if (!info) break;
            const parent = path.slice(0, Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1);
            await extract(parent);
            if (!outcome) break;
        }
        return;
    }
    if (!compress && paths.length === 1 && extensions.includes(paths[0].split('.').pop()!.toLowerCase())) await loadArchive(paths[0]);
    else { sources = [...new Set(paths)]; if (compress) format = 'zip'; page = 'create'; render(); }
}
async function receivePending() { const requests = await invoke<{paths:string[];compress:boolean;extract:boolean}[]>('pending_files'); for (const request of requests) await handlePaths(request.paths, request.compress, request.extract); }
document.addEventListener('keydown', ev => { if (!(ev.metaKey || ev.ctrlKey)) return; if (ev.key.toLowerCase() === 'o') { ev.preventDefault(); void chooseArchive(); } else if (ev.key.toLowerCase() === 'n') { ev.preventDefault(); page = 'create'; render(); } });
render();
if (native) {
    void (async () => {
        os = await invoke<string>('platform');
        appVersion = await getVersion();
        checkForUpdates = initializeUpdater({isBusy: () => busy || loading, setInstalling: value => { updating = value; busy = value; render(); }, notify});
        await listen<string>('job-phase', ev => { jobTitle = ev.payload; progress = null; renderJob(); });
        await listen<Progress>('progress', ev => { progress = ev.payload; renderJob(); });
        await listen('exit-blocked', () => notify('작업을 취소하거나 완료한 뒤 앱을 종료해주세요.'));
        await listen('open-files', async () => { await receivePending(); });
        await getCurrentWebview().onDragDropEvent(ev => { const overlay = document.querySelector('#drop-overlay')!; if (ev.payload.type === 'over' || ev.payload.type === 'enter') overlay.classList.add('visible'); else { overlay.classList.remove('visible'); if (ev.payload.type === 'drop') void handlePaths(ev.payload.paths); } });
        render(); await receivePending();
        void checkForUpdates();
        setInterval(() => { void checkForUpdates?.(); }, 6 * 60 * 60 * 1000);
    })().catch(error => notify(`앱 초기화 오류: ${String(error)}`, true));
}
