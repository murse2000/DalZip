#include <windows.h>
#include <shobjidl.h>
#include <shlwapi.h>
#include <atomic>
#include <filesystem>
#include <string>
#include <vector>
#include <algorithm>
#include <cwctype>

// 선택된 파일 배열을 받아 메뉴 제목을 동적으로 구성합니다.
static const CLSID CLSID_DalZip = {0x627ed496,0x2f22,0x459e,{0xa0,0xec,0xd2,0x45,0x9e,0x46,0xc1,0x91}};
static const CLSID CLSID_DalZipExtract = {0x627ed496,0x2f22,0x459e,{0xa0,0xec,0xd2,0x45,0x9e,0x46,0xc1,0x92}};
static std::atomic<long> objects{0};
static std::wstring executable() {
    wchar_t buffer[32768]; DWORD bytes = sizeof(buffer);
    if (RegGetValueW(HKEY_CURRENT_USER, L"Software\\DalZip", L"Executable", RRF_RT_REG_SZ, nullptr, buffer, &bytes) != ERROR_SUCCESS) return {};
    return buffer;
}
static HRESULT paths(IShellItemArray* items, std::vector<std::wstring>& result) {
    if (!items) return E_INVALIDARG;
    DWORD count = 0; HRESULT hr = items->GetCount(&count); if (FAILED(hr)) return hr;
    for (DWORD i = 0; i < count; ++i) {
        IShellItem* item = nullptr; hr = items->GetItemAt(i, &item); if (FAILED(hr)) return hr;
        PWSTR path = nullptr; hr = item->GetDisplayName(SIGDN_FILESYSPATH, &path); item->Release();
        if (FAILED(hr)) return hr; result.emplace_back(path); CoTaskMemFree(path);
    }
    return result.empty() ? E_INVALIDARG : S_OK;
}
static bool archives(const std::vector<std::wstring>& selected) {
    const std::vector<std::wstring> formats = {L".zip", L".zipx", L".7z", L".rar", L".tar", L".gz", L".tgz", L".bz2", L".tbz2", L".xz", L".txz", L".zst"};
    return !selected.empty() && std::all_of(selected.begin(), selected.end(), [&](const auto& path) {
        auto attributes = GetFileAttributesW(path.c_str());
        auto extension = std::filesystem::path(path).extension().wstring();
        std::transform(extension.begin(), extension.end(), extension.begin(), towlower);
        return attributes != INVALID_FILE_ATTRIBUTES && !(attributes & FILE_ATTRIBUTE_DIRECTORY) && std::find(formats.begin(), formats.end(), extension) != formats.end();
    });
}
// Windows 명령행 인자 규칙에 따라 따옴표와 마지막 역슬래시를 이스케이프합니다.
static std::wstring quote(const std::wstring& value) {
    std::wstring out = L"\""; size_t slashes = 0;
    for (wchar_t c : value) {
        if (c == L'\\') { ++slashes; continue; }
        if (c == L'"') { out.append(slashes * 2 + 1, L'\\'); out += c; }
        else { out.append(slashes, L'\\'); out += c; }
        slashes = 0;
    }
    out.append(slashes * 2, L'\\'); out += L'"'; return out;
}
class Command final : public IExplorerCommand {
    std::atomic<ULONG> references{1};
public:
    bool extract;
    explicit Command(bool unpack) : extract(unpack) { ++objects; } ~Command() { --objects; }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id, void** out) override {
        if (!out) return E_POINTER; *out = nullptr;
        if (id == IID_IUnknown || id == IID_IExplorerCommand) { *out = static_cast<IExplorerCommand*>(this); AddRef(); return S_OK; } return E_NOINTERFACE;
    }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++references; }
    ULONG STDMETHODCALLTYPE Release() override { ULONG n = --references; if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE GetTitle(IShellItemArray* items, LPWSTR* title) override {
        if (extract) return SHStrDupW(L"별도 폴더에 압축 풀기", title);
        std::vector<std::wstring> selected; HRESULT hr = paths(items, selected); if (FAILED(hr)) return SHStrDupW(L"DalZip으로 압축하기", title);
        std::filesystem::path first(selected.front());
        const auto attributes = GetFileAttributesW(selected.front().c_str());
        auto name = (attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_DIRECTORY)) ? first.filename().wstring() : first.stem().wstring();
        return SHStrDupW((name + L".zip으로 압축하기").c_str(), title);
    }
    HRESULT STDMETHODCALLTYPE GetIcon(IShellItemArray*, LPWSTR* icon) override { return SHStrDupW((executable() + L",0").c_str(), icon); }
    HRESULT STDMETHODCALLTYPE GetToolTip(IShellItemArray*, LPWSTR* text) override { return SHStrDupW(extract ? L"각 압축 파일 옆의 별도 폴더에 해제합니다" : L"선택한 파일과 폴더를 DalZip으로 압축합니다", text); }
    HRESULT STDMETHODCALLTYPE GetCanonicalName(GUID* id) override { if (!id) return E_POINTER; *id = extract ? CLSID_DalZipExtract : CLSID_DalZip; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetState(IShellItemArray* items, BOOL, EXPCMDSTATE* state) override { if (!state) return E_POINTER; std::vector<std::wstring> selected; *state = SUCCEEDED(paths(items, selected)) && (!extract || archives(selected)) ? ECS_ENABLED : ECS_HIDDEN; return S_OK; }
    HRESULT STDMETHODCALLTYPE Invoke(IShellItemArray* items, IBindCtx*) override {
        std::vector<std::wstring> selected; HRESULT hr = paths(items, selected); if (FAILED(hr)) return hr;
        if (extract && !archives(selected)) return E_INVALIDARG;
        auto exe = executable(); if (exe.empty()) return HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND);
        std::wstring line = quote(exe) + (extract ? L" --extract" : L" --compress"); for (const auto& path : selected) line += L" " + quote(path);
        if (line.size() >= 32767) return HRESULT_FROM_WIN32(ERROR_FILENAME_EXCED_RANGE);
        STARTUPINFOW startup{}; startup.cb = sizeof(startup); PROCESS_INFORMATION process{};
        if (!CreateProcessW(exe.c_str(), line.data(), nullptr, nullptr, FALSE, 0, nullptr, nullptr, &startup, &process)) return HRESULT_FROM_WIN32(GetLastError());
        CloseHandle(process.hThread); CloseHandle(process.hProcess); return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetFlags(EXPCMDFLAGS* flags) override { if (!flags) return E_POINTER; *flags = ECF_DEFAULT; return S_OK; }
    HRESULT STDMETHODCALLTYPE EnumSubCommands(IEnumExplorerCommand** commands) override { if (commands) *commands = nullptr; return E_NOTIMPL; }
};
class Factory final : public IClassFactory {
    std::atomic<ULONG> references{1};
public:
    bool extract;
    explicit Factory(bool unpack) : extract(unpack) { ++objects; } ~Factory() { --objects; }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id, void** out) override { if (!out) return E_POINTER; *out = nullptr; if (id == IID_IUnknown || id == IID_IClassFactory) { *out = static_cast<IClassFactory*>(this); AddRef(); return S_OK; } return E_NOINTERFACE; }
    ULONG STDMETHODCALLTYPE AddRef() override { return ++references; }
    ULONG STDMETHODCALLTYPE Release() override { ULONG n = --references; if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer, REFIID id, void** out) override { if (outer) return CLASS_E_NOAGGREGATION; auto* command = new(std::nothrow) Command(extract); if (!command) return E_OUTOFMEMORY; HRESULT hr = command->QueryInterface(id, out); command->Release(); return hr; }
    HRESULT STDMETHODCALLTYPE LockServer(BOOL lock) override { objects += lock ? 1 : -1; return S_OK; }
};
extern "C" HRESULT __stdcall DllGetClassObject(REFCLSID clsid, REFIID id, void** out) { if (clsid != CLSID_DalZip && clsid != CLSID_DalZipExtract) return CLASS_E_CLASSNOTAVAILABLE; auto* factory = new(std::nothrow) Factory(clsid == CLSID_DalZipExtract); if (!factory) return E_OUTOFMEMORY; HRESULT hr = factory->QueryInterface(id, out); factory->Release(); return hr; }
extern "C" HRESULT __stdcall DllCanUnloadNow() { return objects == 0 ? S_OK : S_FALSE; }
