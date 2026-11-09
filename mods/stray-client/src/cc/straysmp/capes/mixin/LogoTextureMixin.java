package cc.straysmp.capes.mixin;

import java.io.IOException;
import java.io.InputStream;

import cc.straysmp.capes.StrayCapes;
import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.renderer.texture.MipmapStrategy;
import net.minecraft.client.renderer.texture.TextureContents;
import net.minecraft.client.resources.metadata.texture.TextureMetadataSection;
import net.minecraft.server.packs.resources.ResourceManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Loading screen: show the StraySMP logo instead of "Mojang Studios".
 * The texture uses the same layout as mojangstudios.png (512×512, left half of the wide
 * logo on top, right half below), so the vanilla drawing code works unchanged.
 */
@Mixin(targets = "net.minecraft.client.gui.screens.LoadingOverlay$LogoTexture")
public abstract class LogoTextureMixin {
    @Inject(method = "loadContents", at = @At("HEAD"), cancellable = true)
    private void strayclient$logo(ResourceManager resources, CallbackInfoReturnable<TextureContents> cir) {
        try (InputStream in = StrayCapes.class.getResourceAsStream("/assets/strayclient/splash.png")) {
            if (in == null) return; // keep the vanilla logo
            NativeImage image = NativeImage.read(in);
            cir.setReturnValue(new TextureContents(image, new TextureMetadataSection(true, true, MipmapStrategy.MEAN, 0.0F)));
        } catch (IOException e) {
            StrayCapes.LOG.warn("Could not load the StraySMP splash logo", e);
        }
    }
}
