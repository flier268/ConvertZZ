using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Pipes;
using System.Linq;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using CommunityToolkit.Mvvm.Messaging;
using ConvertZZ.Core.Helpers;
using ConvertZZ.Core.Messages;
using ConvertZZ.Moudle;
using ConvertZZ.ViewModels;

namespace ConvertZZ.Views
{
    /// <summary>
    /// MainWindow.xaml 的互動邏輯
    /// </summary>
    public partial class MainWindow : Window
    {
        private List<Moudle.HotKey> hotKeys = new();
        private CancellationTokenSource Cancellation = new();
        private MainWindowViewModel MainWindowViewModel { get; }

        public MainWindow()
        {
            InitializeComponent();
            MainWindowViewModel = (MainWindowViewModel)DataContext;
            App.NIcon.MouseClick += NIcon_MouseClick;
            if (0 < App.Settings.PositionX && App.Settings.PositionX < SystemParameters.WorkArea.Width)
                Left = App.Settings.PositionX;
            if (0 < App.Settings.PositionY && App.Settings.PositionY < SystemParameters.WorkArea.Height)
                Top = App.Settings.PositionY;
            RegAllHotkey();
            ServerThread();
            RegistMessenger();
        }

        private async void ServerThread()
        {
            while (!Cancellation.IsCancellationRequested)
            {
                try
                {
                    using NamedPipeServerStream pipeServer = new("ConvertZZ_Pipe", PipeDirection.InOut);
                    await pipeServer.WaitForConnectionAsync(Cancellation.Token);
                    Console.WriteLine("Client connected.");
                    StreamString ss = new(pipeServer);
                    string[] Args = (await ss.ReadStringAsync()).Split('|');

                    Window_DialogHost window_DialogHost = new(Args[0] == "/file" ? EMode.File_FileName : EMode.AutioTag, Args.Skip(1).ToArray());
                    window_DialogHost.Show();

                    await ss.WriteStringAsync("ACK");
                    pipeServer.WaitForPipeDrain();
                    pipeServer.Close();
                }
                catch (IOException e)
                {
                    Console.WriteLine("ERROR: {0}", e.Message);
                }
            }
        }

        private void RegistMessenger()
        {
            WeakReferenceMessenger.Default.Register<WindowShowDialogMessage>(this, (r, m) =>
            {
                m.Reply(m.Window.ShowDialog());
            });
            WeakReferenceMessenger.Default.Register<WindowShowMessage>(this, (r, m) =>
            {
                m.Reply(m.Window.ShowDialog());
            });
            WeakReferenceMessenger.Default.Register<DialogHostMessage>(this, (r, m) =>
            {
                Window_DialogHost Window_DialogHost = new(m.Mode, m.AudioFormat);
                Window_DialogHost.Show();
            });
        }

        private void NIcon_MouseClick(object sender, System.Windows.Forms.MouseEventArgs e)
        {
            if (e.Button == System.Windows.Forms.MouseButtons.Right)
            {
                ContextMenu NotifyIconMenu = (ContextMenu)this.FindResource("NotifyIconMenu");
                NotifyIconMenu.IsOpen = true;
            }
        }

        private void RegHotkey(Feature feature)
        {
            if (!feature.Enable) return;
            KeyModifier keyModifier = KeyModifier.None;
            feature.Modift.Split(',').ToList().ForEach(x => keyModifier |= (KeyModifier)Enum.Parse(typeof(KeyModifier), x.Trim()));
            if (feature.Command is not null)
            {
                hotKeys.Add(new Moudle.HotKey((Key)Enum.Parse(typeof(Key), feature.Key), keyModifier, (hotKey) =>
                {
                    if (App.Settings.HotKey.AutoCopy)
                        ClipBoardHelper.Copy(hotKey.Key, hotKey.KeyModifiers);
                    MessengerHelper.Send(feature.Command.Command, feature.Command?.CommandParameter);
                    if (App.Settings.HotKey.AutoPaste)
                        ClipBoardHelper.Paste();
                }).Regist(out _));
            }
        }

        public void RegAllHotkey()
        {
            RegHotkey(App.Settings.HotKey.Feature1);
            RegHotkey(App.Settings.HotKey.Feature2);
            RegHotkey(App.Settings.HotKey.Feature3);
            RegHotkey(App.Settings.HotKey.Feature4);
        }

        public void UnRegAllHotkey()
        {
            hotKeys.ForEach(x => x.Dispose());
            hotKeys.Clear();
        }

        private Point pointNow = new();
        private bool leftDown = false;

        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.LeftButton == MouseButtonState.Pressed)
            {
                pointNow = new Point(Left, Top);
                leftDown = true;
                this.DragMove();
            }
        }

        private void Exit_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
            App.NIcon.Visible = false;
            App.NIcon.Dispose();
            Environment.Exit(0);
        }

        private void Window_MouseUp(object sender, MouseButtonEventArgs e)
        {
            ContextMenu NotifyIconMenu = (ContextMenu)this.FindResource("NotifyIconMenu");
            e.Handled = true;
            switch (e.ChangedButton)
            {
                case MouseButton.Left:
                    {
                        if (Left == pointNow.X && Top == pointNow.Y && leftDown)
                        {
                            if (Keyboard.IsKeyDown(Key.LeftCtrl) || Keyboard.IsKeyDown(Key.RightCtrl))
                                MessengerHelper.Send(App.Settings.QuickStart.LeftClick_Ctrl.Command);
                            else if (Keyboard.IsKeyDown(Key.LeftAlt) || Keyboard.IsKeyDown(Key.RightAlt))
                                MessengerHelper.Send(App.Settings.QuickStart.LeftClick_Alt.Command);
                            else if (Keyboard.IsKeyDown(Key.LeftShift) || Keyboard.IsKeyDown(Key.RightShift))
                                MessengerHelper.Send(App.Settings.QuickStart.LeftClick_Shift.Command);
                            else
                                e.Handled = false;
                        }
                        else
                            e.Handled = false;
                    }
                    break;

                case MouseButton.Right:
                    {
                        if (Keyboard.IsKeyDown(Key.LeftCtrl) || Keyboard.IsKeyDown(Key.RightCtrl))
                            MessengerHelper.Send(App.Settings.QuickStart.RightClick_Ctrl.Command);
                        else if (Keyboard.IsKeyDown(Key.LeftAlt) || Keyboard.IsKeyDown(Key.RightAlt))
                            MessengerHelper.Send(App.Settings.QuickStart.RightClick_Alt.Command);
                        else if (Keyboard.IsKeyDown(Key.LeftShift) || Keyboard.IsKeyDown(Key.RightShift))
                            MessengerHelper.Send(App.Settings.QuickStart.RightClick_Shift.Command);
                        else
                            e.Handled = false;
                    }
                    break;

                default:
                    e.Handled = false;
                    break;
            }
            leftDown = false;
        }

        private DragDropKeyStates dragDropKeyStates;

        private void Window_DragEnter(object sender, DragEventArgs e)
        {
            //紀錄拖曳進來時的按鍵
            dragDropKeyStates = e.KeyStates;
        }

        private void Window_Drop(object sender, DragEventArgs e)
        {
            if (e.Data.GetDataPresent(DataFormats.FileDrop))
            {
                /*
                string[] files = (string[])e.Data.GetData(DataFormats.FileDrop);
                //減去輔助鍵，得到現在是左鍵還是右鍵
                dragDropKeyStates -= e.KeyStates;
                Pages.Page_File page_File = new Pages.Page_File();
                page_File.Button_Convert_Click(null, null);
                if (dragDropKeyStates == DragDropKeyStates.LeftMouseButton)
                {
                    switch (e.KeyStates)
                    {
                        case DragDropKeyStates.ControlKey:
                            MenuItem_Click(new MenuItem { Uid = App.Settings.QuickStart.LeftDrop_Ctrl }, null);
                            break;

                        case DragDropKeyStates.ShiftKey:
                            MenuItem_Click(new MenuItem { Uid = App.Settings.QuickStart.LeftDrop_Shift }, null);
                            break;

                        case DragDropKeyStates.AltKey:
                            MenuItem_Click(new MenuItem { Uid = App.Settings.QuickStart.LeftDrop_Alt }, null);
                            break;
                    }
                }
                else if (dragDropKeyStates == DragDropKeyStates.RightMouseButton)
                {
                    switch (e.KeyStates)
                    {
                        case DragDropKeyStates.ControlKey:
                            MenuItem_Click(new MenuItem { Uid = App.Settings.QuickStart.RightDrop_Ctrl }, null);
                            break;

                        case DragDropKeyStates.ShiftKey:
                            MenuItem_Click(new MenuItem { Uid = App.Settings.QuickStart.RightDrop_Shift }, null);
                            break;

                        case DragDropKeyStates.AltKey:
                            MenuItem_Click(new MenuItem { Uid = App.Settings.QuickStart.RightDrop_Alt }, null);
                            break;
                    }
                }*/
            }
            else if (e.Data.GetDataPresent(DataFormats.UnicodeText))
            {
                dragDropKeyStates -= e.KeyStates;
                string s = (string)e.Data.GetData(DataFormats.UnicodeText);
                if (dragDropKeyStates == DragDropKeyStates.LeftMouseButton)
                {
                    switch (e.KeyStates)
                    {
                        case DragDropKeyStates.ControlKey:
                            MessengerHelper.Send(App.Settings.QuickStart.LeftDrop_Ctrl.Command);
                            break;

                        case DragDropKeyStates.ShiftKey:
                            MessengerHelper.Send(App.Settings.QuickStart.LeftDrop_Shift.Command);
                            break;

                        case DragDropKeyStates.AltKey:
                            MessengerHelper.Send(App.Settings.QuickStart.LeftDrop_Alt.Command);
                            break;
                    }
                }
                else if (dragDropKeyStates == DragDropKeyStates.RightMouseButton)
                {
                    switch (e.KeyStates)
                    {
                        case DragDropKeyStates.ControlKey:
                            MessengerHelper.Send(App.Settings.QuickStart.RightDrop_Ctrl.Command);
                            break;

                        case DragDropKeyStates.ShiftKey:
                            MessengerHelper.Send(App.Settings.QuickStart.RightDrop_Shift.Command);
                            break;

                        case DragDropKeyStates.AltKey:
                            MessengerHelper.Send(App.Settings.QuickStart.RightDrop_Alt.Command);
                            break;
                    }
                }
            }
            else
            {
                var g = e.Data.GetFormats(true);
                foreach (var h in g)
                {
                    object ss = e.Data.GetData(h);
                    if (ss != null)
                    {
                    }
                }
                string s = (string)e.Data.GetData(DataFormats.EnhancedMetafile);
            }
        }

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            Cancellation.Cancel();
            App.Settings.PositionX = Left;
            App.Settings.PositionY = Top;
            App.Save();
            UnRegAllHotkey();
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            if (!App.Settings.AssistiveTouch)
                this.Visibility = Visibility.Hidden;
        }
    }
}