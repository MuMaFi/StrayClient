package cc.straysmp.loading.mixin;

import java.io.IOException;
import java.io.InputStream;

import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.renderer.texture.MipmapStrategy;
import net.minecraft.client.renderer.texture.TextureContents;
import net.minecraft.client.resources.metadata.texture.TextureMetadataSection;
import net.minecraft.server.packs.resources.ResourceManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Forge/NeoForge loading screen: the Stray Client logo instead of "Mojang Studios". */
@Mixin(targets = "net.minecraft.client.gui.screens.LoadingOverlay$LogoTexture")
public abstract class LogoTextureMixin {
    @Inject(method = "loadContents", at = @At("HEAD"), cancellable = true, require = 0)
    private void strayloading$logo(ResourceManager resources, CallbackInfoReturnable<TextureContents> cir) {
        try (InputStream in = LogoTextureMixin.class.getResourceAsStream("/assets/strayclient/splash.png")) {
            if (in == null) return; // keep the vanilla logo
            NativeImage image = NativeImage.read(in);
            cir.setReturnValue(new TextureContents(image, new TextureMetadataSection(true, true, MipmapStrategy.MEAN, 0.0F)));
        } catch (IOException e) {
            System.err.println("[Stray Client] could not load the loading-screen logo: " + e);
        }
    }
}
