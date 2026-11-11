package cc.straysmp.capes.menu;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Properties;

import cc.straysmp.capes.StrayCapes;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.gui.screens.Screen;

/**
 * Live module toggles. Keystrokes and PvP Plus are separate mods, so they are reached through
 * reflection and only offered when they are loaded.
 */
public final class Modules {
    private static final String KS_CONFIG = "net.marblock.keystrokes.config.KeystrokesConfig";
    private static final String PVP_CONFIG = "de.pvpplus.Config";
    private static final String PVP_SCREEN = "de.pvpplus.PvpScreen";

    private Modules() {}

    // ── Keystrokes ──
    static boolean keystrokesLoaded() {
        return FabricLoader.getInstance().isModLoaded("modern-keystrokes") && cls(KS_CONFIG) != null;
    }

    static boolean keystrokes() {
        try {
            return cls(KS_CONFIG).getField("isEnabled").getBoolean(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return false;
        }
    }

    static void setKeystrokes(boolean on) {
        try {
            Class<?> c = cls(KS_CONFIG);
            c.getField("isEnabled").setBoolean(null, on);
            c.getMethod("save").invoke(null);
            Pending.put("keystrokes", Boolean.toString(on));
        } catch (ReflectiveOperationException | RuntimeException e) {
            StrayCapes.LOG.warn("Keystrokes toggle failed", e);
        }
    }

    static int keystrokesX() {
        return ksInt("x", 25);
    }

    static int keystrokesY() {
        return ksInt("y", 10);
    }

    static float keystrokesScale() {
        try {
            return cls(KS_CONFIG).getField("scale").getFloat(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return 1f;
        }
    }

    private static int ksInt(String field, int fallback) {
        try {
            return cls(KS_CONFIG).getField(field).getInt(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return fallback;
        }
    }

    static void setKeystrokesPos(int x, int y) {
        try {
            Class<?> c = cls(KS_CONFIG);
            c.getField("x").setInt(null, x);
            c.getField("y").setInt(null, y);
        } catch (ReflectiveOperationException | RuntimeException e) {
            StrayCapes.LOG.warn("Keystrokes move failed", e);
        }
    }

    /**
     * Default place for Keystrokes. PvP Plus fills the left side (info at the top, armor in the
     * middle), so with it Keystrokes goes to the middle of the right edge.
     */
    static void placeKeystrokesDefault(int guiWidth, int guiHeight) {
        if (!pvpLoaded()) {
            setKeystrokesPos(25, 10);
            return;
        }
        float scale = Math.max(0.5f, keystrokesScale());
        int w = Math.round(66 * scale), h = Math.round(100 * scale);
        setKeystrokesPos(Math.max(0, guiWidth - w - 6), Math.max(0, (guiHeight - h) / 2));
    }

    private static boolean autoPlaced;

    /**
     * Once per start, in a world: if Keystrokes still sits where the launcher puts it
     * (x=25, y=10 or 80) and PvP Plus is on, move it off the PvP Plus HUD.
     */
    public static void autoPlaceKeystrokes(net.minecraft.client.Minecraft mc) {
        if (autoPlaced || mc.player == null) return;
        autoPlaced = true;
        if (!keystrokesLoaded() || !pvpLoaded()) return;
        if (keystrokesX() != 25 || (keystrokesY() != 10 && keystrokesY() != 80)) return;
        placeKeystrokesDefault(mc.getWindow().getGuiScaledWidth(), mc.getWindow().getGuiScaledHeight());
        saveKeystrokes();
    }

    static void saveKeystrokes() {
        if (!keystrokesLoaded()) return;
        try {
            cls(KS_CONFIG).getMethod("save").invoke(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            StrayCapes.LOG.warn("Keystrokes save failed", e);
        }
    }

    // ── Cape (our own mod) ──
    private static Path capeConfig() {
        return FabricLoader.getInstance().getConfigDir().resolve("stray-capes").resolve("config.properties");
    }

    private static Properties capeProps() {
        Properties p = new Properties();
        try (InputStream in = Files.newInputStream(capeConfig())) {
            p.load(in);
        } catch (IOException ignored) {
        }
        return p;
    }

    static boolean capeOn() {
        return "local".equals(capeProps().getProperty("mode", "account").trim());
    }

    static boolean hasCapeFile() {
        return !capeProps().getProperty("cape", "").trim().isEmpty();
    }

    static void setCape(boolean on) {
        Properties p = capeProps();
        String mode = on ? "local" : "account";
        p.setProperty("mode", mode);
        try {
            Files.createDirectories(capeConfig().getParent());
            try (OutputStream out = Files.newOutputStream(capeConfig())) {
                p.store(out, "Stray Capes (managed by Stray Client)");
            }
            Pending.put("cape", mode);
        } catch (IOException e) {
            StrayCapes.LOG.warn("Could not save cape config", e);
        }
    }

    // ── PvP Plus ──
    static boolean pvpLoaded() {
        return FabricLoader.getInstance().isModLoaded("pvpplus") && cls(PVP_CONFIG) != null;
    }

    private static Object pvpConfig() throws ReflectiveOperationException {
        return cls(PVP_CONFIG).getMethod("get").invoke(null);
    }

    static boolean pvp(String field) {
        try {
            Object cfg = pvpConfig();
            return cfg.getClass().getField(field).getBoolean(cfg);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return false;
        }
    }

    static void setPvp(String field, boolean on) {
        try {
            Object cfg = pvpConfig();
            Field f = cfg.getClass().getField(field);
            f.setBoolean(cfg, on);
            cls(PVP_CONFIG).getMethod("save").invoke(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            StrayCapes.LOG.warn("PvP Plus toggle {} failed", field, e);
        }
    }

    /** PvP Plus' own settings screen (all of its options), or null. */
    static Screen pvpScreen(Screen parent) {
        try {
            Class<?> c = cls(PVP_SCREEN);
            return c == null ? null : (Screen) c.getConstructor(Screen.class).newInstance(parent);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return null;
        }
    }

    private static Class<?> cls(String name) {
        try {
            return Class.forName(name, true, Modules.class.getClassLoader());
        } catch (ClassNotFoundException | LinkageError e) {
            return null;
        }
    }
}
