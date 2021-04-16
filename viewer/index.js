import * as viewer from './pkg/viewer.js';

document.addEventListener('DOMContentLoaded', async event => {
  await viewer.default();

  let model = await fetch('./pkg/model.fm');
  if (!model.ok) {
    throw 'failed to fetch a model';
  }

  let view = viewer.Viewer.fromModelBuffer(model.arrayBuffer());

  let doc = document.documentElement;
  let canvas = document.getElementById('canvas');
  canvas.setAttribute('height', doc.clientHeight);
  canvas.setAttribute('width', doc.clientWidth);

  view.start(canvas);
});
