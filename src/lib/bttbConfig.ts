export interface BttbCoordinates {
  x: number;
  y: number;
  z: number;
}

export interface BttbLocation extends BttbCoordinates {
  number: string;
}

export interface BttbPlayer {
  name: string;
  locations: BttbLocation[];
}

export interface BttbConfig {
  language: "Chinese" | "English";
  players: BttbPlayer[];
  returnEnabled: boolean;
  returnLocation: BttbCoordinates;
  adminEnabled: boolean;
  adminPlayers: string[];
}

export function createDefaultBttbConfig(): BttbConfig {
  return {
    language: "Chinese",
    players: [{ name: "example_name", locations: [{ number: "1", x: 0, y: 60, z: 0 }] }],
    returnEnabled: false,
    returnLocation: { x: 0, y: 60, z: 0 },
    adminEnabled: false,
    adminPlayers: [],
  };
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

function integer(value: unknown, field: string): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < -2147483648 || value > 2147483647) {
    throw new Error(`${field} 必须是整数`);
  }
  return value;
}

function boolean(value: unknown, field: string, fallback = false): boolean {
  if (value === undefined) return fallback;
  if (typeof value !== "boolean") throw new Error(`${field} 必须是 true 或 false`);
  return value;
}

function coordinates(value: unknown, field: string, defaults: BttbCoordinates): BttbCoordinates {
  if (value === undefined) return { ...defaults };
  if (!isRecord(value)) throw new Error(`${field} 必须是坐标对象`);
  return {
    x: integer(value.x, `${field}.x`),
    y: integer(value.y, `${field}.y`),
    z: integer(value.z, `${field}.z`),
  };
}

export function parseBttbConfig(content: string): BttbConfig {
  let root: unknown;
  try {
    root = JSON.parse(content);
  } catch {
    throw new Error("配置文件不是有效的 JSON");
  }
  if (!isRecord(root)) throw new Error("配置文件根节点必须是对象");
  if (!isRecord(root.players)) {
    throw new Error("未找到新版 players 配置；请先让 BTTB 完成旧配置迁移");
  }

  const players: BttbPlayer[] = Object.entries(root.players).map(([name, playerValue]) => {
    if (!isRecord(playerValue) || !Array.isArray(playerValue.locations)) {
      throw new Error(`玩家 ${name || "（空名称）"} 缺少 locations 数组`);
    }
    const locations = playerValue.locations.map((locationValue, index) => {
      if (!isRecord(locationValue)) throw new Error(`玩家 ${name} 的位置 ${index + 1} 格式无效`);
      return {
        number: typeof locationValue.number === "string" ? locationValue.number : String(locationValue.number ?? ""),
        x: integer(locationValue.x, `${name} 位置 ${index + 1} 的 X`),
        y: integer(locationValue.y, `${name} 位置 ${index + 1} 的 Y`),
        z: integer(locationValue.z, `${name} 位置 ${index + 1} 的 Z`),
      };
    });
    return { name, locations };
  });

  const returnValue = root.return;
  if (returnValue !== undefined && !isRecord(returnValue)) throw new Error("return 必须是对象");
  const returnRecord = isRecord(returnValue) ? returnValue : {};
  const adminValue = root.admin;
  if (adminValue !== undefined && !isRecord(adminValue)) throw new Error("admin 必须是对象");
  const adminRecord = isRecord(adminValue) ? adminValue : {};
  const adminPlayers = adminRecord.players === undefined ? [] : adminRecord.players;
  if (!Array.isArray(adminPlayers) || adminPlayers.some((player) => typeof player !== "string")) {
    throw new Error("admin.players 必须是玩家名称数组");
  }
  const languageValue = root.language === undefined ? "Chinese" : root.language;
  if (typeof languageValue !== "string" || !["chinese", "english"].includes(languageValue.toLowerCase())) {
    throw new Error("language 只能是 Chinese 或 English");
  }
  const language: BttbConfig["language"] = languageValue.toLowerCase() === "english" ? "English" : "Chinese";

  const config: BttbConfig = {
    language,
    players,
    returnEnabled: boolean(returnRecord.enabled, "return.enabled"),
    returnLocation: coordinates(returnRecord.location, "return.location", { x: 0, y: 60, z: 0 }),
    adminEnabled: boolean(adminRecord.enabled, "admin.enabled"),
    adminPlayers: [...adminPlayers],
  };
  const validationError = validateBttbConfig(config);
  if (validationError) throw new Error(validationError);
  return config;
}

export function serializeBttbConfig(config: BttbConfig): string {
  const players = Object.fromEntries(config.players.map((player) => [
    player.name,
    { locations: player.locations.map((location) => ({ ...location })) },
  ]));
  return `${JSON.stringify({
    language: config.language,
    players,
    return: {
      enabled: config.returnEnabled,
      location: { ...config.returnLocation },
    },
    admin: {
      enabled: config.adminEnabled,
      players: [...config.adminPlayers],
    },
  }, null, 2)}\n`;
}

export function validateBttbConfig(config: BttbConfig): string {
  if (config.language !== "Chinese" && config.language !== "English") return "语言设置无效";
  if (config.players.length === 0) return "至少需要配置一名玩家";
  const playerNames = new Set<string>();
  for (const player of config.players) {
    const name = player.name.trim();
    if (!name) return "玩家名称不能为空";
    if (name !== player.name) return `玩家名称不能包含首尾空格：${player.name}`;
    const key = name.toLowerCase();
    if (playerNames.has(key)) return `玩家名称重复：${name}`;
    playerNames.add(key);
    if (player.locations.length === 0) return `玩家 ${name} 至少需要一个珍珠按钮位置`;
    const numbers = new Set<string>();
    for (const location of player.locations) {
      if (!/^[1-9]\d*$/.test(location.number)) return `${name} 的位置编号必须是正整数`;
      if (Number(location.number) > 2147483647) return `${name} 的位置编号超出整数范围`;
      if (numbers.has(location.number)) return `${name} 的位置编号 ${location.number} 重复`;
      numbers.add(location.number);
      for (const [axis, value] of Object.entries({ X: location.x, Y: location.y, Z: location.z })) {
        if (!Number.isInteger(value)) return `${name} 的位置 ${location.number}：${axis} 必须是整数`;
        if (value < -2147483648 || value > 2147483647) {
          return `${name} 的位置 ${location.number}：${axis} 超出整数范围`;
        }
      }
    }
  }
  for (const [axis, value] of Object.entries({ X: config.returnLocation.x, Y: config.returnLocation.y, Z: config.returnLocation.z })) {
    if (!Number.isInteger(value)) return `返回点 ${axis} 必须是整数`;
    if (value < -2147483648 || value > 2147483647) return `返回点 ${axis} 超出整数范围`;
  }
  if (config.adminPlayers.length > 3) return "管理员最多只能配置 3 名";
  const admins = new Set<string>();
  for (const player of config.adminPlayers) {
    const name = player.trim();
    if (!name) return "管理员名称不能为空";
    if (name !== player) return `管理员名称不能包含首尾空格：${player}`;
    const key = name.toLowerCase();
    if (admins.has(key)) return `管理员名称重复：${name}`;
    admins.add(key);
  }
  return "";
}
