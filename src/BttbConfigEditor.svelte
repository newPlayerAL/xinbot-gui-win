<script lang="ts">
  import type { BttbConfig } from "./lib/bttbConfig";

  export let config: BttbConfig;
  export let disabled = false;
  export let onChange: (next: BttbConfig) => void;

  const clone = (): BttbConfig => structuredClone(config);

  function update(mutator: (next: BttbConfig) => void) {
    const next = clone();
    mutator(next);
    onChange(next);
  }

  function nextPlayerName(): string {
    const used = new Set(config.players.map((player) => player.name.toLowerCase()));
    let index = 1;
    while (used.has(`player${index}`.toLowerCase())) index += 1;
    return `Player${index}`;
  }

  function nextLocationNumber(playerIndex: number): string {
    const used = new Set(config.players[playerIndex].locations.map((location) => location.number));
    let index = 1;
    while (used.has(String(index))) index += 1;
    return String(index);
  }

  function numberInput(event: Event): number {
    return (event.currentTarget as HTMLInputElement).valueAsNumber;
  }
</script>

<div class="bttb-editor">
  <section class="bttb-section compact">
    <div class="section-copy"><strong>插件语言</strong><small>控制 BTTB 的控制台和游戏内消息</small></div>
    <select value={config.language} disabled={disabled} onchange={(event) => update((next) => { next.language = (event.currentTarget as HTMLSelectElement).value as BttbConfig["language"]; })}>
      <option value="Chinese">简体中文</option>
      <option value="English">English</option>
    </select>
  </section>

  <section class="bttb-section">
    <div class="section-heading">
      <div class="section-copy"><strong>玩家与珍珠按钮</strong><small>每名玩家可配置多个编号位置；“back”默认使用 1 号</small></div>
      <button type="button" disabled={disabled} onclick={() => update((next) => next.players.push({ name: nextPlayerName(), locations: [{ number: "1", x: 0, y: 60, z: 0 }] }))}>＋ 添加玩家</button>
    </div>
    <div class="player-list">
      {#each config.players as player, playerIndex}
        <article class="player-card">
          <div class="player-heading">
            <label><span>玩家名称</span><input value={player.name} disabled={disabled} oninput={(event) => update((next) => { next.players[playerIndex].name = (event.currentTarget as HTMLInputElement).value; })} /></label>
            <button class="remove" type="button" disabled={disabled || config.players.length <= 1} onclick={() => update((next) => next.players.splice(playerIndex, 1))}>删除玩家</button>
          </div>
          <div class="location-list">
            {#each player.locations as location, locationIndex}
              <div class="location-row">
                <label class="number"><span>编号</span><input value={location.number} inputmode="numeric" disabled={disabled} oninput={(event) => update((next) => { next.players[playerIndex].locations[locationIndex].number = (event.currentTarget as HTMLInputElement).value; })} /></label>
                <label><span>X</span><input type="number" value={location.x} disabled={disabled} oninput={(event) => update((next) => { next.players[playerIndex].locations[locationIndex].x = numberInput(event); })} /></label>
                <label><span>Y</span><input type="number" value={location.y} disabled={disabled} oninput={(event) => update((next) => { next.players[playerIndex].locations[locationIndex].y = numberInput(event); })} /></label>
                <label><span>Z</span><input type="number" value={location.z} disabled={disabled} oninput={(event) => update((next) => { next.players[playerIndex].locations[locationIndex].z = numberInput(event); })} /></label>
                <button class="remove icon" type="button" title="删除位置" aria-label="删除位置" disabled={disabled || player.locations.length <= 1} onclick={() => update((next) => next.players[playerIndex].locations.splice(locationIndex, 1))}>×</button>
              </div>
            {/each}
          </div>
          <button class="add-location" type="button" disabled={disabled} onclick={() => update((next) => next.players[playerIndex].locations.push({ number: nextLocationNumber(playerIndex), x: 0, y: 60, z: 0 }))}>＋ 添加按钮位置</button>
        </article>
      {/each}
    </div>
  </section>

  <section class="bttb-section split">
    <div>
      <div class="section-copy"><strong>点击后返回</strong><small>完成按钮操作后回到统一返回点</small></div>
      <label class="toggle"><input type="checkbox" checked={config.returnEnabled} disabled={disabled} onchange={(event) => update((next) => { next.returnEnabled = (event.currentTarget as HTMLInputElement).checked; })} /><span>{config.returnEnabled ? "已启用" : "未启用"}</span></label>
    </div>
    <div class="coordinate-row">
      <label><span>X</span><input type="number" value={config.returnLocation.x} disabled={disabled} oninput={(event) => update((next) => { next.returnLocation.x = numberInput(event); })} /></label>
      <label><span>Y</span><input type="number" value={config.returnLocation.y} disabled={disabled} oninput={(event) => update((next) => { next.returnLocation.y = numberInput(event); })} /></label>
      <label><span>Z</span><input type="number" value={config.returnLocation.z} disabled={disabled} oninput={(event) => update((next) => { next.returnLocation.z = numberInput(event); })} /></label>
    </div>
  </section>

  <section class="bttb-section">
    <div class="section-heading">
      <div class="section-copy"><strong>游戏内管理员</strong><small>允许通过私聊 @bttb 管理；插件最多支持 3 名</small></div>
      <label class="toggle"><input type="checkbox" checked={config.adminEnabled} disabled={disabled} onchange={(event) => update((next) => { next.adminEnabled = (event.currentTarget as HTMLInputElement).checked; })} /><span>{config.adminEnabled ? "已启用" : "未启用"}</span></label>
    </div>
    <div class="admin-list">
      {#each config.adminPlayers as player, index}
        <div class="admin-row">
          <input value={player} placeholder="管理员玩家名" disabled={disabled} oninput={(event) => update((next) => { next.adminPlayers[index] = (event.currentTarget as HTMLInputElement).value; })} />
          <button class="remove icon" type="button" title="删除管理员" aria-label="删除管理员" disabled={disabled} onclick={() => update((next) => next.adminPlayers.splice(index, 1))}>×</button>
        </div>
      {/each}
      {#if config.adminPlayers.length < 3}
        <button class="add-location" type="button" disabled={disabled} onclick={() => update((next) => next.adminPlayers.push(""))}>＋ 添加管理员</button>
      {/if}
    </div>
  </section>
</div>

<style>
  .bttb-editor { width: 100%; min-width: 0; display: grid; gap: 8px; margin-top: 9px; }
  .bttb-section { width: 100%; min-width: 0; padding: 9px; border: 1px solid rgba(115,147,255,.16); border-radius: 8px; background: rgba(8,13,21,.28); }
  .bttb-section.compact,.section-heading,.player-heading,.bttb-section.split { display: flex; align-items: center; justify-content: space-between; gap: 9px; }
  .section-copy { min-width: 0; }.section-copy strong,.section-copy small { display: block; }.section-copy strong { color: #b9c5d4; font-size: 8px; }.section-copy small { margin-top: 3px; color: #617084; font-size: 7px; line-height: 1.4; }
  select, input { height: 28px; border: 1px solid #293548; border-radius: 6px; outline: 0; background: rgba(7,12,19,.62); color: #aab8c9; font-size: 8px; }
  select { min-width: 105px; padding: 0 7px; } input { width: 100%; padding: 0 7px; } input:focus,select:focus { border-color: rgba(83,216,228,.45); box-shadow: 0 0 0 2px rgba(83,216,228,.06); }
  button { min-height: 26px; padding: 0 8px; border: 1px solid rgba(83,216,228,.2); border-radius: 6px; background: rgba(83,216,228,.06); color: #8fcdd2; font-size: 7px; font-weight: 700; cursor: pointer; white-space: nowrap; } button:disabled { opacity: .4; cursor: not-allowed; }
  .player-list { min-width: 0; display: grid; gap: 7px; margin-top: 8px; }.player-card { min-width: 0; padding: 8px; border: 1px solid #263246; border-radius: 7px; background: rgba(4,7,12,.3); }.player-heading label { min-width: 0; flex: 1; }
  label > span { display: block; margin-bottom: 3px; color: #68768a; font-size: 7px; }.remove { border-color: rgba(255,117,135,.18); background: rgba(255,117,135,.05); color: #c77b86; }.remove.icon { width: 27px; min-width: 27px; padding: 0; font-size: 13px; }
  .location-list { display: grid; gap: 5px; margin-top: 7px; }.location-row { display: grid; grid-template-columns: 58px repeat(3,minmax(48px,1fr)) 27px; align-items: end; gap: 5px; }.location-row .number input { text-align: center; }.location-row input,.coordinate-row input { font-family: "Cascadia Mono",Consolas,monospace; }
  .add-location { width: 100%; margin-top: 6px; border-style: dashed; }
  .bttb-section.split > div { min-width: 0; flex: 1; }.coordinate-row { min-width: 0; display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 5px; }.coordinate-row label { min-width: 0; }
  .toggle { display: inline-flex; align-items: center; gap: 6px; margin-top: 7px; color: #8392a8; font-size: 7px; }.toggle input { width: 13px; height: 13px; accent-color: #55cbd5; }.toggle span { margin: 0; }
  .admin-list { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); gap: 6px; margin-top: 8px; }.admin-row { display: grid; grid-template-columns: minmax(0,1fr) 27px; gap: 4px; }.admin-list > .add-location { margin-top: 0; }
  :global(.light) .bttb-section { border-color: #d7dfea; background: #f7f9fc; }:global(.light) .player-card { border-color: #d8e0ea; background: #fff; }:global(.light) .section-copy strong { color: #415168; }:global(.light) .section-copy small,:global(.light) label > span { color: #8490a2; }:global(.light) input,:global(.light) select { border-color: #cdd6e2; background: #fff; color: #415168; }:global(.light) button { color: #167d8b; border-color: #b8dce0; background: #eefafb; }:global(.light) .remove { color: #ae4c5c; border-color: #efcbd1; background: #fff2f4; }
</style>
