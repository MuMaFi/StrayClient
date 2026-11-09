package cc.straysmp.capes.mixin;

import cc.straysmp.capes.StrayCapes;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.core.ClientAsset;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.player.PlayerSkin;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Swaps the cape (and elytra) texture of your own player for the local PNG. */
@Mixin(AbstractClientPlayer.class)
public abstract class AbstractClientPlayerMixin {
    @Inject(method = "getSkin", at = @At("RETURN"), cancellable = true)
    private void straycapes$cape(CallbackInfoReturnable<PlayerSkin> cir) {
        if ((Object) this != Minecraft.getInstance().player) return;
        String mode = StrayCapes.mode();
        PlayerSkin skin = cir.getReturnValue();
        if ("none".equals(mode)) {
            cir.setReturnValue(new PlayerSkin(skin.body(), null, null, skin.model(), skin.secure()));
            return;
        }
        if (!"local".equals(mode)) return;
        Identifier id = StrayCapes.capeTexture();
        if (id == null) return;
        ClientAsset.Texture cape = new ClientAsset.ResourceTexture(id);
        cir.setReturnValue(new PlayerSkin(skin.body(), cape, cape, skin.model(), skin.secure()));
    }
}
