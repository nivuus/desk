# Lit le crete-metre de CHAQUE point de terminaison de rendu, independamment
# de notre agent : c'est l'instrument qui dit ou va reellement le son.
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
[ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class MMDeviceEnumerator { }
[ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IMMDeviceEnumerator { int EnumAudioEndpoints(int f,int s,out IMMDeviceCollection c); int GetDefaultAudioEndpoint(int f,int r,out IMMDevice d); }
[ComImport, Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IMMDeviceCollection { int GetCount(out int c); int Item(int i,out IMMDevice d); }
[ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IMMDevice { int Activate(ref Guid iid,int ctx,IntPtr p,[MarshalAs(UnmanagedType.IUnknown)] out object o);
  int OpenPropertyStore(int a,out IPropertyStore s); int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id); int GetState(out int st); }
[ComImport, Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IPropertyStore { int GetCount(out int c); int GetAt(int i,out PROPERTYKEY k); int GetValue(ref PROPERTYKEY k,out PROPVARIANT v); }
[ComImport, Guid("C02216F6-8C67-4B5B-9D00-D008E73E0064"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface IAudioMeterInformation { int GetPeakValue(out float p); }
[StructLayout(LayoutKind.Sequential)] struct PROPERTYKEY { public Guid fmtid; public int pid; }
[StructLayout(LayoutKind.Explicit)] struct PROPVARIANT { [FieldOffset(0)] public short vt; [FieldOffset(8)] public IntPtr p; }
public static class Metre {
  public static string Lire(int tours) {
    var e=(IMMDeviceEnumerator)(new MMDeviceEnumerator());
    IMMDeviceCollection col; e.EnumAudioEndpoints(0,1,out col); int n; col.GetCount(out n);
    var noms=new string[n]; var metres=new IAudioMeterInformation[n];
    var iid=typeof(IAudioMeterInformation).GUID;
    for(int i=0;i<n;i++){ IMMDevice d; col.Item(i,out d); IPropertyStore ps; d.OpenPropertyStore(0,out ps);
      var k=new PROPERTYKEY(); k.fmtid=new Guid("a45c254e-df1c-4efd-8020-67d146a850e0"); k.pid=14;
      PROPVARIANT v; ps.GetValue(ref k,out v); noms[i]=Marshal.PtrToStringUni(v.p);
      object o; d.Activate(ref iid,1,IntPtr.Zero,out o); metres[i]=(IAudioMeterInformation)o; }
    var max=new float[n]; var sb=new System.Text.StringBuilder();
    for(int t=0;t<tours;t++){ for(int i=0;i<n;i++){ float p; metres[i].GetPeakValue(out p); if(p>max[i]) max[i]=p; }
      System.Threading.Thread.Sleep(100); }
    for(int i=0;i<n;i++) sb.AppendLine("METRE crete=" + max[i].ToString("F6") + "  " + noms[i]);
    return sb.ToString();
  }
}
'@
[Metre]::Lire(80)
