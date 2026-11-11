package cc.straysmp.capes.mixin;

import cc.straysmp.capes.menu.Hosting;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Client tick for the "Host" request from the launcher. */
@Mixin(Minecraft.class)
public abstract class MinecraftMixin {
    @Inject(method = "tick", at = @At("TAIL"))
    private void strayclient$tick(CallbackInfo ci) {
        Hosting.tick((Minecraft) (Object) this);
    }
}
