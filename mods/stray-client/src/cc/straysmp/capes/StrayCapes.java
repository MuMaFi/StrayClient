package cc.straysmp.capes;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Properties;

import com.mojang.blaze3d.platform.NativeImage;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Stray Capes – shows a cape from a local PNG on your own player.
 *
 * The Stray Client launcher writes {@code config/stray-capes/config.properties}
 * ({@code mode=local|account|none}, {@code cape=<file>.png}) and copies the PNG into
 * {@code config/stray-capes/capes/}. The file is re-read while the game runs, so a cape
 * picked in the launcher shows up within a second.
 *
 * Based on the idea of Cape Switcher by IcyCroww (MIT).
 */
public final class StrayCapes implements ClientModInitializer {
    public static final String MOD_ID = "strayclient";
    public static final Logger LOG = LoggerFactory.getLogger("StrayClient");

    private static final long CHECK_INTERVAL_MS = 1000;

    private static Path dir;
    private static long lastCheck;
    private static long configStamp = -1;
    private static long capeStamp = -1;
    private static String mode = "account";
    private static String capeFile = "";
    private static Identifier texture;

    @Override
    public void onInitializeClient() {
        dir = FabricLoader.getInstance().getConfigDir().resolve("stray-capes");
        try {
            Files.createDirectories(dir.resolve("capes"));
        } catch (IOException e) {
            LOG.warn("Could not create {}", dir, e);
        }
        LOG.info("Stray Capes ready ({})", dir);
    }

    /** "local", "account" (keep the Mojang cape) or "none". */
    public static String mode() {
        refresh();
        return mode;
    }

    /** Texture of the local cape, or null if there is none. Must be called on the render thread. */
    public static Identifier capeTexture() {
        refresh();
        return texture;
    }

    private static void refresh() {
        if (dir == null) return;
        long now = System.currentTimeMillis();
        if (now - lastCheck < CHECK_INTERVAL_MS) return;
        lastCheck = now;

        Path cfg = dir.resolve("config.properties");
        long stamp = stamp(cfg);
        if (stamp != configStamp) {
            configStamp = stamp;
            Properties p = new Properties();
            if (stamp >= 0) {
                try (InputStream in = Files.newInputStream(cfg)) {
                    p.load(in);
                } catch (IOException e) {
                    LOG.warn("Could not read {}", cfg, e);
                }
            }
            mode = p.getProperty("mode", "account").trim();
            String file = p.getProperty("cape", "").trim();
            if (!file.equals(capeFile)) {
                capeFile = file;
                capeStamp = -1; // force a reload of the image
            }
        }

        if (!"local".equals(mode) || capeFile.isEmpty()) {
            unload();
            return;
        }
        Path png = dir.resolve("capes").resolve(capeFile).normalize();
        if (!png.startsWith(dir)) { // never read outside our folder
            unload();
            return;
        }
        long s = stamp(png);
        if (s == capeStamp) return;
        capeStamp = s;
        unload();
        if (s < 0) {
            LOG.warn("Cape file {} not found", png);
            return;
        }
        try (InputStream in = Files.newInputStream(png)) {
            NativeImage image = NativeImage.read(in);
            Identifier id = Identifier.fromNamespaceAndPath(MOD_ID, "cape/" + sanitize(capeFile));
            Minecraft.getInstance().getTextureManager().register(id, new DynamicTexture(() -> "Stray cape " + capeFile, image));
            texture = id;
            LOG.info("Loaded cape {} ({}x{})", capeFile, image.getWidth(), image.getHeight());
        } catch (IOException | RuntimeException e) {
            LOG.warn("Could not load cape {}", png, e);
        }
    }

    private static void unload() {
        if (texture != null) {
            Minecraft.getInstance().getTextureManager().release(texture);
            texture = null;
        }
    }

    private static long stamp(Path p) {
        try {
            return Files.getLastModifiedTime(p).toMillis() ^ Files.size(p);
        } catch (IOException e) {
            return -1;
        }
    }

    static String sanitize(String name) {
        return name.toLowerCase(java.util.Locale.ROOT).replaceAll("[^a-z0-9._-]", "_");
    }
}
