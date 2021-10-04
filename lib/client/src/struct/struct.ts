export class Struct {
  /**
   * source data
   */
  data: object;
  /**
   * Buffer Layout
   */
  layout: any;

  constructor(data: object, layout: any) {
    this.data = data;
    this.layout = layout;
  }

  public toBuffer(): Buffer {
    const data = Buffer.alloc(this.layout.span);
    const length = this.layout.encode(this.data, data);
    return data.slice(0, length);
  }

  public get() {
    return this.data;
  }
}
