# LE JUGE de E2 : ouvre un point de terminaison de CAPTURE designe par
# sous-chaine, exactement comme le ferait une application Windows qui choisit
# ce microphone, et rend la CRETE et la FREQUENCE DOMINANTE.
#
# ⚠️ On juge a la FREQUENCE DOMINANTE, jamais a un compte d'octets ni a une
# crete seule : doctrine payee en D7 (une piste peut voir bytesReceived croitre
# avec un spectre a -1000 dB).
#
# ⚠️ Le script rend AUSSI le format qu'il a obtenu (GetMixFormat) et calcule la
# frequence sur CE taux : un juge qui supposerait 48000 sur un endpoint a
# 44100 rendrait une frequence fausse de 8,8 % sans que rien ne le dise.
#
# La plomberie COM se recopie de C:\dev\micro-format-e1.ps1 (E1) ; le
# crete-metre de abis-metre.ps1 en est le voisin.
param(
  [string]$Prefixe   = 'CABLE Output',
  [int]   $Secondes  = 8,
  [string]$Journal   = 'C:\dev\micro-ecoute-e2.log'
)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)

$src = @'
using System;
using System.Runtime.InteropServices;
using System.Collections.Generic;
public static class Ecoute {
  [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class Enumr { }
  [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IMMDeviceEnumerator {
    int EnumAudioEndpoints(int df, int mask, out IMMDeviceCollection col);
    int GetDefaultAudioEndpoint(int df, int role, out IMMDevice dev);
  }
  [Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IMMDeviceCollection { int GetCount(out int n); int Item(int i, out IMMDevice d); }
  [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IMMDevice {
    int Activate(ref Guid iid, int ctx, IntPtr p, [MarshalAs(UnmanagedType.IUnknown)] out object o);
    int OpenPropertyStore(int a, out IPropertyStore s);
    int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
    int GetState(out int st);
  }
  [Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IPropertyStore {
    int GetCount(out int n); int GetAt(int i, out PropertyKey k);
    int GetValue(ref PropertyKey k, out PropVariant v);
    int SetValue(ref PropertyKey k, ref PropVariant v); int Commit();
  }
  [StructLayout(LayoutKind.Sequential)] public struct PropertyKey { public Guid fmtid; public int pid; }
  [StructLayout(LayoutKind.Explicit)] public struct PropVariant {
    [FieldOffset(0)] public short vt; [FieldOffset(8)] public IntPtr p;
  }
  // L'ORDRE des methodes est celui de la vtable : toute omission decale tout
  // ce qui suit, et GetService appellerait SetEventHandle.
  [Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IAudioClient {
    int Initialize(int mode, int flags, long buf, long per, IntPtr fmt, IntPtr sess);
    int GetBufferSize(out uint n);
    int GetStreamLatency(out long l);
    int GetCurrentPadding(out uint p);
    int IsFormatSupported(int mode, IntPtr fmt, out IntPtr closest);
    int GetMixFormat(out IntPtr fmt);
    int GetDevicePeriod(out long def, out long min);
    int Start();
    int Stop();
    int Reset();
    int SetEventHandle(IntPtr h);
    int GetService(ref Guid iid, [MarshalAs(UnmanagedType.IUnknown)] out object o);
  }
  [Guid("C8ADBD64-E71E-48a0-A4DE-185C395CD317"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
  interface IAudioCaptureClient {
    int GetBuffer(out IntPtr data, out uint frames, out uint flags, out ulong devPos, out ulong qpcPos);
    int ReleaseBuffer(uint frames);
    int GetNextPacketSize(out uint frames);
  }
  static Guid IID_IAudioClient        = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
  static Guid IID_IAudioCaptureClient = new Guid("C8ADBD64-E71E-48a0-A4DE-185C395CD317");
  static Guid SUB_IEEE_FLOAT          = new Guid("00000003-0000-0010-8000-00aa00389b71");
  static Guid SUB_PCM                 = new Guid("00000001-0000-0010-8000-00aa00389b71");
  static PropertyKey PKEY_Name = new PropertyKey {
    fmtid = new Guid("a45c254e-df1c-4efd-8020-67d146a850e0"), pid = 14 };

  static string Nom(IMMDevice d) {
    try { IPropertyStore ps; d.OpenPropertyStore(0, out ps);
          PropVariant pv; var k = PKEY_Name; ps.GetValue(ref k, out pv);
          if (pv.vt == 31) return Marshal.PtrToStringUni(pv.p); } catch {}
    return "?";
  }

  public static string[] Mesurer(string prefixe, int secondes) {
    var res = new List<string>();
    var e = (IMMDeviceEnumerator)(new Enumr());
    IMMDeviceCollection col;
    e.EnumAudioEndpoints(1 /* eCapture */, 1 /* ACTIVE */, out col);
    int n; col.GetCount(out n);
    IMMDevice cible = null; string nomCible = null, idCible = null;
    for (int i = 0; i < n; i++) {
      IMMDevice d; col.Item(i, out d);
      string nom = Nom(d); string id; d.GetId(out id);
      res.Add("  capture[" + i + "] = " + nom + "  ||  " + id);
      if (cible == null && nom.IndexOf(prefixe, StringComparison.OrdinalIgnoreCase) >= 0) {
        cible = d; nomCible = nom; idCible = id;
      }
    }
    if (cible == null) { res.Add("AUCUN endpoint de capture ne contient '" + prefixe + "'"); return res.ToArray(); }
    res.Add("CIBLE = " + nomCible + "  ||  " + idCible);

    object o; cible.Activate(ref IID_IAudioClient, 1 /* CLSCTX_ALL */, IntPtr.Zero, out o);
    var ac = (IAudioClient)o;
    IntPtr pf; int hr = ac.GetMixFormat(out pf);
    if (hr != 0) { res.Add("GetMixFormat HRESULT 0x" + hr.ToString("X8")); return res.ToArray(); }
    short tag = Marshal.ReadInt16(pf, 0);
    short ch  = Marshal.ReadInt16(pf, 2);
    int  rate = Marshal.ReadInt32(pf, 4);
    short bits= Marshal.ReadInt16(pf, 14);
    short cb  = Marshal.ReadInt16(pf, 16);
    Guid sub = Guid.Empty;
    if (tag == unchecked((short)0xFFFE) && cb >= 22) {
      byte[] g = new byte[16]; Marshal.Copy(new IntPtr(pf.ToInt64() + 24), g, 0, 16); sub = new Guid(g);
    }
    bool flottant = (tag == 3) || (tag == unchecked((short)0xFFFE) && sub == SUB_IEEE_FLOAT);
    bool entier16 = (bits == 16) && ((tag == 1) || (tag == unchecked((short)0xFFFE) && sub == SUB_PCM));
    res.Add("FORMAT tag=" + (tag == unchecked((short)0xFFFE) ? "EXTENSIBLE" : tag.ToString())
            + " HZ=" + rate + " canaux=" + ch + " bits=" + bits
            + (sub == Guid.Empty ? "" : " sous-format=" + sub.ToString()));
    if (!(flottant && bits == 32) && !entier16) {
      res.Add("FORMAT NON DECODABLE PAR CE JUGE (ni f32 ni pcm16) — mesure non prise");
      return res.ToArray();
    }

    hr = ac.Initialize(0 /* SHARED */, 0, 10000000L /* 1 s */, 0, pf, IntPtr.Zero);
    Marshal.FreeCoTaskMem(pf);
    if (hr != 0) { res.Add("Initialize HRESULT 0x" + hr.ToString("X8")); return res.ToArray(); }
    object oc; hr = ac.GetService(ref IID_IAudioCaptureClient, out oc);
    if (hr != 0) { res.Add("GetService HRESULT 0x" + hr.ToString("X8")); return res.ToArray(); }
    var cc = (IAudioCaptureClient)oc;
    hr = ac.Start();
    if (hr != 0) { res.Add("Start HRESULT 0x" + hr.ToString("X8")); return res.ToArray(); }

    var gauche = new List<float>(rate * secondes + 1024);
    long tramesSilence = 0, tramesTotal = 0, paquets = 0;
    var t0 = DateTime.UtcNow;
    while ((DateTime.UtcNow - t0).TotalSeconds < secondes) {
      uint dispo; if (cc.GetNextPacketSize(out dispo) != 0) break;
      if (dispo == 0) { System.Threading.Thread.Sleep(5); continue; }
      IntPtr data; uint frames, flags; ulong dp, qp;
      if (cc.GetBuffer(out data, out frames, out flags, out dp, out qp) != 0) break;
      paquets++;
      bool silence = (flags & 0x2) != 0;   // AUDCLNT_BUFFERFLAGS_SILENT
      if (silence) tramesSilence += frames;
      for (uint t = 0; t < frames; t++) {
        float v = 0f;
        if (!silence) {
          if (flottant) v = BitConverter.ToSingle(BitConverter.GetBytes(Marshal.ReadInt32(data, (int)(t * ch) * 4)), 0);
          else v = Marshal.ReadInt16(data, (int)(t * ch) * 2) / 32768f;
        }
        gauche.Add(v);
      }
      tramesTotal += frames;
      cc.ReleaseBuffer(frames);
    }
    ac.Stop();

    res.Add("PAQUETS=" + paquets + " TRAMES=" + tramesTotal + " TRAMES_SILENCE=" + tramesSilence);
    if (gauche.Count == 0) { res.Add("CRETE=0.000000"); res.Add("FREQUENCE=0.0 (aucune trame)"); return res.ToArray(); }
    float crete = 0f;
    for (int i = 0; i < gauche.Count; i++) { float a = Math.Abs(gauche[i]); if (a > crete) crete = a; }
    res.Add("CRETE=" + crete.ToString("F6", System.Globalization.CultureInfo.InvariantCulture));

    // Balayage de Goertzel : la resolution d'un bloc de N echantillons vaut
    // rate/N, on balaie plus fin qu'elle et l'on retient le maximum.
    int N = Math.Min(32768, gauche.Count);
    int debut = (gauche.Count - N) / 2;
    double meilleure = 0, puissanceMax = -1;
    for (int f = 80; f <= 4000; f++) {
      double w = 2.0 * Math.PI * f / rate, coeff = 2.0 * Math.Cos(w);
      double s1 = 0, s2 = 0;
      for (int i = 0; i < N; i++) { double s = gauche[debut + i] + coeff * s1 - s2; s2 = s1; s1 = s; }
      double p = s1 * s1 + s2 * s2 - coeff * s1 * s2;
      if (p > puissanceMax) { puissanceMax = p; meilleure = f; }
    }
    double amplitude = 2.0 * Math.Sqrt(Math.Max(puissanceMax, 0)) / N;
    res.Add("FREQUENCE=" + meilleure.ToString("F1", System.Globalization.CultureInfo.InvariantCulture)
            + " AMPLITUDE=" + amplitude.ToString("F6", System.Globalization.CultureInfo.InvariantCulture)
            + " (balayage 80..4000 Hz sur " + N + " echantillons a " + rate + " Hz)");
    return res.ToArray();
  }
}
'@

$flux = New-Object System.IO.StreamWriter($Journal, $false, (New-Object System.Text.UTF8Encoding($false)))
function Dire([string]$l) { Write-Output $l; $flux.WriteLine($l); $flux.Flush() }
try {
  Dire ("JUGE micro-ecoute-e2 : prefixe='{0}' secondes={1} debut={2}" -f $Prefixe, $Secondes, (Get-Date).ToString('o'))
  Add-Type -TypeDefinition $src -Language CSharp -ErrorAction Stop
  [Ecoute]::Mesurer($Prefixe, $Secondes) | ForEach-Object { Dire $_ }
  Dire ("FIN {0}" -f (Get-Date).ToString('o'))
} catch { Dire ('ECHEC : ' + $_.Exception.ToString()) } finally { $flux.Close() }
