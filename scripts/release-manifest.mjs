import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const directory = process.argv[2] ?? 'artifacts';
const version = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json')).version;
const shellResource = JSON.parse(fs.readFileSync('src-tauri/tauri.windows.conf.json')).bundle.resources['resources/DalZipShell.dll'];
if (shellResource !== `shell/DalZipShell-${version}.dll`) throw new Error('Windows 확장 DLL 버전이 앱 버전과 다릅니다.');
const platforms = {};
for (const [platform, name] of [
    ['darwin-aarch64', `DalZip-${version}-arm64.app.tar.gz`],
    ['windows-x86_64', `DalZip-${version}-x64-setup.exe`],
]) {
    if (!fs.statSync(path.join(directory,name)).size) throw new Error('빈 업데이트 파일입니다.');
    const signature = fs.readFileSync(path.join(directory,name+'.sig'),'utf8').trim();
    if (!signature) throw new Error('업데이트 서명이 없습니다.');
    platforms[platform] = {url:`https://github.com/murse2000/DalZip/releases/download/v${version}/${name}`, signature};
}
const notesFile = `releases/v${version}.md`;
const manifest = {version, notes:fs.readFileSync(notesFile,'utf8'), pub_date:new Date().toISOString(), platforms};
fs.writeFileSync(path.join(directory,'latest.json'), JSON.stringify(manifest,null,2)+'\n');
const files = [`DalZip-${version}-arm64.dmg`, `DalZip-${version}-arm64.app.tar.gz`, `DalZip-${version}-arm64.app.tar.gz.sig`, `DalZip-${version}-x64-setup.exe`, `DalZip-${version}-x64-setup.exe.sig`, 'latest.json'];
fs.writeFileSync(path.join(directory,'SHA256SUMS.txt'), files.map(name => `${createHash('sha256').update(fs.readFileSync(path.join(directory,name))).digest('hex')}  ${name}`).join('\n')+'\n');
console.log(`v${version} 업데이트 피드와 체크섬 생성 완료`);
