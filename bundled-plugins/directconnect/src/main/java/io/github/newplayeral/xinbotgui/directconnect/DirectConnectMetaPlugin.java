/*
 *   Copyright (C) 2026 newPlayerAL
 *
 *   This program is free software: you can redistribute it and/or modify
 *   it under the terms of the GNU General Public License as published by
 *   the Free Software Foundation, either version 3 of the License, or
 *   (at your option) any later version.
 *
 *   This program is distributed in the hope that it will be useful,
 *   but WITHOUT ANY WARRANTY; without even the implied warranty of
 *   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *   GNU General Public License for more details.
 *
 *   You should have received a copy of the GNU General Public License
 *   along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

package io.github.newplayeral.xinbotgui.directconnect;

import org.geysermc.mcprotocollib.protocol.packet.ingame.clientbound.ClientboundLoginPacket;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import xin.bbtt.mcbot.Bot;
import xin.bbtt.mcbot.LoginFlow.LoginFlow;
import xin.bbtt.mcbot.Server;
import xin.bbtt.mcbot.plugin.MetaPlugin;

import javax.naming.directory.Attribute;
import javax.naming.directory.Attributes;
import javax.naming.directory.DirContext;
import javax.naming.directory.InitialDirContext;
import java.net.InetSocketAddress;
import java.net.SocketAddress;
import java.util.Hashtable;

/**
 * A generic {@link MetaPlugin} that lets xinbot connect to <i>any</i> server, driven entirely by
 * JVM system properties the launcher sets when it spawns the bot process. It carries no
 * server-specific logic, so a single jar works for every server the GUI manages.
 *
 * <h3>Inputs (system properties)</h3>
 * <ul>
 *   <li>{@code xinbot.server.host} — required, the server hostname/IP.</li>
 *   <li>{@code xinbot.server.port} — optional. If set, used verbatim. If absent, an SRV lookup of
 *       {@code _minecraft._tcp.<host>} is attempted, falling back to port 25565.</li>
 *   <li>{@code xinbot.login.template} — optional, default {@code "/login {password}"}. The command
 *       sent once the bot joins, with {@code {password}} expanded from the account password in
 *       config.conf. Auto-login is skipped for online-mode accounts or when the password is empty.</li>
 * </ul>
 */
public class DirectConnectMetaPlugin implements MetaPlugin {

    private static final Logger log = LoggerFactory.getLogger("DirectConnect");

    public static final String PROP_HOST = "xinbot.server.host";
    public static final String PROP_PORT = "xinbot.server.port";
    public static final String PROP_LOGIN_TEMPLATE = "xinbot.login.template";
    public static final String DEFAULT_LOGIN_TEMPLATE = "/login {password}";
    private static final int DEFAULT_PORT = 25565;

    @Override
    public SocketAddress getServerSocketAddress() {
        String host = System.getProperty(PROP_HOST);
        if (host == null || host.isBlank()) {
            throw new IllegalStateException(
                    "DirectConnect: system property " + PROP_HOST + " is not set; the launcher must "
                            + "pass the server host via -D" + PROP_HOST + "=<host>");
        }
        host = host.trim();

        String portProp = System.getProperty(PROP_PORT);
        if (portProp != null && !portProp.isBlank()) {
            int port = Integer.parseInt(portProp.trim());
            log.info("DirectConnect: connecting to {}:{} (explicit port)", host, port);
            return new InetSocketAddress(host, port);
        }

        InetSocketAddress srv = resolveSrv(host);
        if (srv != null) {
            log.info("DirectConnect: connecting to {}:{} (SRV {})", srv.getHostString(), srv.getPort(), host);
            return srv;
        }
        log.info("DirectConnect: connecting to {}:{} (default port)", host, DEFAULT_PORT);
        return new InetSocketAddress(host, DEFAULT_PORT);
    }

    @Override
    public Server getServer(ClientboundLoginPacket loginPacket) {
        // This plugin is server-agnostic; once the login packet arrives we are in the play phase.
        return Server.Game;
    }

    @Override
    public void onEnable() {
        setupAutoLogin();
    }

    @Override
    public void onLoad() { }

    @Override
    public void onDisable() { }

    @Override
    public void onUnload() { }

    /**
     * Wires up an optional secondary login (e.g. AuthMe {@code /login <password>}) using the
     * declarative {@link LoginFlow} API. No-op for online-mode accounts or when no password is set.
     */
    private void setupAutoLogin() {
        var account = Bot.INSTANCE.getConfig().getConfigData().getAccount();
        if (account.isOnlineMode()) {
            return; // Online (Microsoft) accounts authenticate via the game session, not a chat command.
        }
        String password = account.getPassword();
        if (password == null || password.isEmpty()) {
            return; // No secondary-login password configured.
        }
        String template = System.getProperty(PROP_LOGIN_TEMPLATE, DEFAULT_LOGIN_TEMPLATE);
        if (template.isBlank()) {
            return; // Explicitly disabled.
        }

        LoginFlow flow = LoginFlow.builder(Bot.INSTANCE::sendChatMessage)
                .templateExpander(t -> t.replace("{password}", password))
                .step(ClientboundLoginPacket.class)
                    .describe("send login command on join")
                    .match(p -> true)
                    .then(template)
                    .add()
                .cooldown(3000)
                .build();
        Bot.INSTANCE.addPacketListener(flow, this);
        log.info("DirectConnect: auto-login enabled (template: {})", template);
    }

    /** True if {@code host} is a raw IPv4/IPv6 literal (so an SRV lookup would be pointless). */
    private static boolean isIpLiteral(String host) {
        if (host.indexOf(':') >= 0) {
            return true; // IPv6 literal.
        }
        return host.matches("\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}");
    }

    /**
     * Resolves a Minecraft SRV record ({@code _minecraft._tcp.<host>}) via DNS, mirroring how a
     * vanilla client locates a server when no explicit port is given. Returns {@code null} on any
     * failure so the caller can fall back to the default port.
     */
    private static InetSocketAddress resolveSrv(String host) {
        if (isIpLiteral(host)) {
            return null; // SRV records only make sense for hostnames, not raw IPs.
        }
        try {
            Hashtable<String, String> env = new Hashtable<>();
            env.put("java.naming.factory.initial", "com.sun.jndi.dns.DnsContextFactory");
            env.put("java.naming.provider.url", "dns:");
            // Bound the lookup so a missing/misconfigured resolver can't stall the connection.
            env.put("com.sun.jndi.dns.timeout.initial", "3000");
            env.put("com.sun.jndi.dns.timeout.retries", "2");
            DirContext ctx = new InitialDirContext(env);
            try {
                Attributes attrs = ctx.getAttributes("_minecraft._tcp." + host, new String[]{"SRV"});
                Attribute srv = (attrs == null) ? null : attrs.get("SRV");
                if (srv == null || srv.size() == 0) {
                    return null;
                }
                // SRV value: "priority weight port target"
                String[] parts = ((String) srv.get(0)).split("\\s+");
                if (parts.length < 4) {
                    return null;
                }
                int port = Integer.parseInt(parts[2]);
                String target = parts[3];
                if (target.endsWith(".")) {
                    target = target.substring(0, target.length() - 1);
                }
                return new InetSocketAddress(target, port);
            } finally {
                ctx.close();
            }
        } catch (Exception e) {
            return null;
        }
    }
}
