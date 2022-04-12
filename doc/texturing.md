# Documentation for the texturing procedure

| Name                            | Type           | Units    | Valid range               |
| :------------------------------ | :------------- | :------- | :------------------------ |
| patch_spacing                   | f64            | images   | 0.0 <= _ <= 1.0           |
| gutter_size                     | usize          | pixels   |                           |
| image_resolution                | usize          | pixels   |                           |
| selection_cost_limit            | f64            |          | 0.0 <= _                  |
| background.color                | color          |          | 0..=255                   |
| background.deviation            | f64            |          | 0.0 <= _ <= 255*sqrt(8/3) |
| background.dilations            | Vec&lt;f64&gt; | pixels   | _ < 0.0 or 0.0 < _        |
| color_correction_steps          | usize          |          |                           |
| color_correction_final_offset   | bool           |          |                           |
| input_patching_threshold        | f64            | old cost | 1.0 <= _                  |
| selection_corner_radius         | usize          | edges    |                           |
| background_consensus_threshold  | f64            |          | 0.0 <= _ <= 1.0           |
| background_consensus_spread     | usize          | edges    |                           |

## Input selection

The **background detection** step measures the magnitude of the difference between `background.color` and the color of a given image pixel, modulo the brightness component. If the result is below `background.deviation` then the pixel counts preliminarily as part of the background. Since this result may be a little noisy and not perfectly reliable either right at the edge of a scanned object or in certain regions with otherwise confusing colors, morphology operations can be applied to the preliminary results. These are specified as a list of `background.dilations`, in which negative values mean _erosion_: spots of background color that are smaller than a certain radius will not be counted as background; and _dilation_: pixels within a certain radius from the background will be counted as part of it. For instance if `background.dilations = [-1.0, 3.0, -5.0, 10.0]` this means to erode by 1 pixel, then dilate by 3 pixels, then erode by 5 pixels, then dilate by 10 pixels. Finally, a mesh face is classified as being part of the background in a particular image, precisely when the pixel of at least one of its vertices is classified as being part of the background.

Using the background detection results and various other metrics having to do with orientation and alignment with the camera view axis, a **selection cost** is then calculated for each `(image, face)` pair. This cost, when it is not infinite due to hard constraints (i.e., face must be in front of the camera, and must not be occluded), is the cosecant of the angle formed between a face normal and the camera view axis. An optional step is then performed, which rules a pair `(image, face)` as impossible (i.e. having infinite cost) if `(image, face')` is impossible for some `face'` less than `selection_corner_radius` steps (i.e. crossings from face to face via an edge) from `face`. This can be useful when the geometry parameters have some error in them. Finally, **the best image source is chosen for each face**, as determined by the costs just calculated. If the minimum cost of a face exceeds `selection_cost_limit` then the face won't get its texture from any image, because they are all considered too low quality.

Next **input patching** is performed, in order to make it more common for neighbouring faces to have the exact same image source. The mechanisn for this is as follows: Pre-existing patches of faces with a common image source are allowed to steal nearby faces, as long as the cost ratio incurred by this is below `input_patching_threshold`. In other words, when there is a patch of neighbouring faces `A1, A2, A3, ..., An` that all have a common image source `im1` already, and a face `B` neighbouring any one of these, with a texture source `im2`, then input patching will typically change the source of `B` to be `im1` if this is possible without incurring too great a cost. The bigger patches get to act first, and successively smaller patches get their chance to steal from their neighbours until the whole mesh has been traversed. The same face cannot be stolen twice.

Since input patching has a tendency to leak some **background color** into the texture (due to geometry inaccuracies if nothing else), a mandatory step following input patching is to forbid not just `(image, face)` pairs that are background (as determined by the considerations above, involving `background.color`, `background.deviation` and `background.dilations`), but to forbid `(image, face)` for all `image`s and a given `face`, if a proportion of `(image', face)` pairs greater than `background_consensus_threshold` is background. To be precise, this is the proportion between the number of acceptable images that say the face is green, and the total number of acceptable images, where acceptable means `cost < selection_cost_limit`. To avoid more false negatives, the parameter `background_consensus_spread` can also be set. Its effect is then to forbid not just `(image, face)` for all `image` but also `(image, face')` for all `face'` that are sufficiently close to the `face` that was already determined to be part of the background by this kind of consensus.

## Color correction

Now that faces have been assigned their image source, there remains to hide the visible seams resulting from an image source difference. Conceptually, this is done by replacing the texture `img` by `img' := img + u` where `u` is a certain correction term. This term is chosen so as to make neighbouring face colors coincide at face boundaries, and subject to this constraint, a secondary condition is to minimize the surface integral of `||grad(u)||^2`. A piecewise linear discretization is performed, whose level of detail is the same as the given mesh. The resulting sparse linear system of equations is solved by `color_correction_steps` steps of the conjugate gradients method. If there are faces that are without image source, the initial guess at the solution amounts to a simple imputation of missing data. Since the linear operator `grad` has uniform brightness as part of its nullspace, an optional step `color_correction_final_offset` may be enabled to adjust the average brightness of the result to be as close to that of the original as possible.

## Texture atlas baking

Right before texture baking, nearby mesh faces are grouped together to increase the texture atlas density. The resulting patches need to be separated a little to avoid interfering with each other. This is controlled by `patch_spacing`, which is measured relative to the total `image_resolution`.

The baking step itself starts with an empty image. It then pulls pixels, that fall within the predetermined region of a face, from the texture source of that face. To avoid rendering errors, a gutter of size `gutter_size` is added at the end of this, around each patch. This means that some nearby pixels that used to be black will now be filled with nearby color values.
