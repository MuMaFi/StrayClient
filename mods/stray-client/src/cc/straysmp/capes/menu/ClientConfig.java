package cc.straysmp.capes.menu;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Properties;

import cc.straysmp.capes.StrayCapes;
import net.fabricmc.loader.api.FabricLoader;

/** In-game options of the Stray Client mod ({@code config/strayclient/client.properties}). */
public final class ClientConfig {
    private static boolean loaded;
    private static boolean customMenu = true;

    private ClientConfig() {}

    private static Path file() {
        return FabricLoader.getInstance().getConfigDir().resolve("strayclient").resolve("client.properties");
    }

    private static synchronized void load() {
        if (loaded) return;
        loaded = true;
        Properties p = new Properties();
        try (InputStream in = Files.newInputStream(file())) {
            p.load(in);
        } catch (IOException ignored) {
            return;
        }
        customMenu = !"false".equals(p.getProperty("customMenu", "true").trim());
    }

    /** Whether the Stray Client main menu replaces Minecraft's title screen. */
    public static boolean customMenu() {
        load();
        return customMenu;
    }

    public static synchronized void setCustomMenu(boolean on) {
        load();
        customMenu = on;
        Properties p = new Properties();
        p.setProperty("customMenu", Boolean.toString(on));
        try {
            Files.createDirectories(file().getParent());
            try (OutputStream out = Files.newOutputStream(file())) {
                p.store(out, "Stray Client (in-game options)");
            }
        } catch (IOException e) {
            StrayCapes.LOG.warn("Could not save {}", file(), e);
        }
    }
}
