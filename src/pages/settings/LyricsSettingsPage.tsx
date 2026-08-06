import type { ProviderSettings, ProviderStatus } from "../../shared/types";
import { useSettingsContext } from "../settings";
import styles from "../settings.module.scss";
import { SettingsCard, SettingsHeading } from "./components";

function formatTime(value: number | null | undefined) {
  const seconds = Math.max(0, Math.round((value ?? 0) / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

function healthLabel(status?: ProviderStatus) {
  if (!status || status.health === "unknown") return "尚未测试";
  if (status.health === "available") return "可用";
  if (status.health === "degraded") return "部分可用";
  return "不可用";
}

export default function LyricsSettingsPage() {
  const {
    playback,
    lyrics,
    fileInput,
    providerRows,
    providerView,
    testingProvider,
    resettingSection,
    confirmingReset,
    providerDrag,
    savingProviderOrder,
    manualTitle,
    manualArtist,
    manualAlbum,
    manualDuration,
    setManualTitle,
    setManualArtist,
    setManualAlbum,
    setManualDuration,
    saveProviderSettings,
    beginProviderDrag,
    continueProviderDrag,
    finishProviderDrag,
    setProviderDrag,
    providerDragTransform,
    toggleProvider,
    testProvider,
    handleFile,
    manualSearch,
    resetSection,
  } = useSettingsContext();

  const lyricCapabilities = lyrics.document
    ? [
        lyrics.document.tracks.translation ? "有翻译" : "无翻译",
        lyrics.document.tracks.romanization ? "有音译" : "无音译",
        lyrics.document.tracks.original.lines.some((line) => line.words?.length) ? "有逐字时间轴" : "无逐字时间轴",
      ].join(" · ")
    : "关联歌词后会显示翻译、音译和逐字时间轴的可用状态";

  return (
    <>
      <SettingsHeading title="歌词与搜索" description="高匹配同步歌词会自动采用；其他结果会留在首页等待确认。" onReset={() => void resetSection("lyrics")} resetting={resettingSection === "lyrics"} confirming={confirmingReset === "lyrics"} />
      <SettingsCard title="当前歌曲">
        <div className={styles.currentTrack}><div><strong>{playback.snapshot.title ?? "没有正在播放的歌曲"}</strong><small>{playback.snapshot.artist ?? "—"}</small></div><em>{lyrics.document?.metadata.source ?? "未关联歌词"}</em></div>
        <p className={styles.cardHint}>{lyricCapabilities}</p>
        <div className={styles.buttonRow}>
          <button disabled={!lyrics.trackKey || lyrics.searching} onClick={() => void lyrics.search()}>{lyrics.searching ? "搜索中…" : "手动搜索当前歌曲"}</button>
          <button disabled={!lyrics.trackKey} onClick={() => fileInput.current?.click()}>导入 LRC</button>
          <input ref={fileInput} hidden type="file" accept=".lrc,text/plain" onChange={(event) => void handleFile(event.currentTarget.files?.[0])} />
          {lyrics.document && <button className={styles.danger} onClick={() => void lyrics.remove()}>解除关联</button>}
        </div>
        {lyrics.document && <div className={styles.offsetRow}><span>歌词偏移 {lyrics.document.offsetMs > 0 ? "+" : ""}{lyrics.document.offsetMs}ms</span><div><button onClick={() => void lyrics.changeOffset(-100)}>−100</button><button onClick={() => void lyrics.changeOffset(100)}>+100</button><button onClick={() => void lyrics.setOffset(0)}>重置</button></div></div>}
      </SettingsCard>
      <SettingsCard title="修改信息后搜索">
        <form className={styles.searchForm} onSubmit={(event) => { event.preventDefault(); manualSearch(); }}>
          <label><span>歌曲名</span><input value={manualTitle} onChange={(event) => setManualTitle(event.target.value)} /></label>
          <label><span>歌手</span><input value={manualArtist} onChange={(event) => setManualArtist(event.target.value)} /></label>
          <label><span>专辑</span><input value={manualAlbum} onChange={(event) => setManualAlbum(event.target.value)} /></label>
          <label><span>时长（秒）</span><input type="number" min="0" value={manualDuration} onChange={(event) => setManualDuration(event.target.value)} /></label>
          <button type="submit" disabled={!lyrics.trackKey || !manualTitle.trim() || !manualArtist.trim() || lyrics.searching}>搜索全部已启用来源</button>
        </form>
        {lyrics.results.length > 0 && <div className={styles.searchResults}>{lyrics.results.map((result) => <button key={`${result.providerId}:${result.id}`} onClick={() => void lyrics.applyResult(result)}><span><strong>{result.title}</strong><small>{result.artist} · {result.source}{result.durationMs ? ` · ${formatTime(result.durationMs)}` : ""}{result.hasTranslation ? " · 翻译" : ""}{result.hasRomanization ? " · 音译" : ""}{result.hasWordTiming ? " · 逐字" : ""}</small></span><b>{Math.round(result.score * 100)}%</b></button>)}</div>}
      </SettingsCard>
      <SettingsCard title="歌词源优先级" trailing={providerView && <select disabled={savingProviderOrder} value={providerView.settings.mode} onChange={(event) => void saveProviderSettings({ ...providerView.settings, mode: event.target.value as ProviderSettings["mode"] })}><option value="strict">严格优先级</option><option value="smart">智能排序</option></select>}>
        <p className={styles.cardHint}>{providerView?.settings.mode === "smart" ? "智能排序会在分数差距较大时覆盖手动顺序。" : "搜索结果按下列优先级排列；拖动左侧把手可调整并立即保存。"}</p>
        <div className={styles.providers} data-dragging={Boolean(providerDrag)} aria-busy={savingProviderOrder}>{providerView?.settings.providers.map((provider, index) => {
          const status = providerView.statuses.find((item) => item.providerId === provider.id);
          return <div className={styles.provider} data-dragging={providerDrag?.providerId === provider.id} key={provider.id} ref={(element) => { if (element) providerRows.current.set(provider.id, element); else providerRows.current.delete(provider.id); }} style={{ transform: providerDragTransform(index) }}>
            <button type="button" className={styles.dragHandle} disabled={savingProviderOrder} aria-label={`拖动${status?.name ?? provider.id}调整优先级`} onPointerDown={(event) => beginProviderDrag(provider.id, index, event)} onPointerMove={continueProviderDrag} onPointerUp={finishProviderDrag} onPointerCancel={() => setProviderDrag(null)} onLostPointerCapture={() => setProviderDrag(null)}>⠿</button>
            <b>#{index + 1}</b><div><strong>{status?.name ?? provider.id}</strong><small data-health={status?.health ?? "unknown"}>{healthLabel(status)}{status?.message ? ` · ${status.message}` : ""}</small></div>
            <button className={styles.switch} disabled={savingProviderOrder} data-on={provider.enabled} onClick={() => toggleProvider(provider.id)}><span /></button>
            <button disabled={testingProvider === provider.id} onClick={() => void testProvider(provider.id)}>{testingProvider === provider.id ? "测试中" : "测试"}</button>
          </div>;
        })}</div>
      </SettingsCard>
    </>
  );
}
