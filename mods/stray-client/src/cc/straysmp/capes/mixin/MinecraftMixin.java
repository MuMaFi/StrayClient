package cc.straysmp.capes.mixin;

import cc.straysmp.capes.menu.Hosting;
import cc.straysmp.capes.menu.Modules;
import cc.straysmp.capes.menu.StrayTitleScreen;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Client tick: the "Host" request from the launcher and the default Keystrokes place. */
@Mixin(Minecraft.class)
public abstract class MinecraftMixin {
    @Inject(method = "tick", at = @At("TAIL"))
    private void strayclient$tick(CallbackInfo ci) {
        Hosting.tick((Minecraft) (Object) this);
        StrayTitleScreen.tick((Minecraft) (Object) this);
        Modules.autoPlaceKeystrokes((Minecraft) (Object) this);
    }
}
